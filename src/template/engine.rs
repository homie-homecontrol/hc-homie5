use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use minijinja::{Environment, UndefinedBehavior, Value as MjValue};
use serde::Deserialize;
use serde_yaml_ng::{Mapping, Value};

use super::error::{key_to_string, PathSegment, TemplateError, TemplateErrorKind as Kind};
use super::expr::{parse_scalar, when_expression, Piece, Scalar};
use super::registry::{Template, TemplateRegistry};
use crate::query::QueryDefinition;

/// Default limit for nested `use` (template bodies using templates).
pub const DEFAULT_MAX_DEPTH: usize = 32;

/// What the app supplies to an expansion. The library never sees app stores: everything is
/// passed as owned YAML values.
pub trait ExpandContext {
    /// Evaluates a `for_each` query. Order does not matter: the engine sorts the result by
    /// `sort_by`, then by each element's `id` field.
    fn query(&self, query: &QueryDefinition) -> Result<Vec<Value>, String>;

    /// Variables visible to every expression (outermost scope). Called once per expansion.
    ///
    /// App functions registered on [`Engine::env_mut`] can read them through
    /// `minijinja::State::lookup`, for example a `device('id')` function over a devices map.
    fn globals(&self) -> Mapping {
        Mapping::new()
    }

    /// Keys that open a scope. In every mapping that has one of these keys, their values are
    /// expanded first (in the outer scope) and passed to [`ExpandContext::scope_vars`]; the
    /// returned variables are visible in the rest of that mapping and its subtree. Called once
    /// per expansion.
    fn scope_keys(&self) -> Vec<String> {
        Vec::new()
    }

    /// Variables for the subtree of a mapping; `keys` holds the mapping's expanded scope keys
    /// (see [`ExpandContext::scope_keys`]), `path` its location. Inner scopes shadow outer ones.
    fn scope_vars(&self, path: &[PathSegment], keys: &Mapping) -> Result<Option<Mapping>, String> {
        let _ = (path, keys);
        Ok(None)
    }
}

/// A context without queries or variables: every `for_each` yields nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct EmptyContext;

impl ExpandContext for EmptyContext {
    fn query(&self, _query: &QueryDefinition) -> Result<Vec<Value>, String> {
        Ok(Vec::new())
    }
}

/// Template engine: a minijinja environment (strict undefined) plus expansion limits.
///
/// Apps customize the environment once, e.g. `engine.env_mut().add_function("has_property", f)`.
pub struct Engine {
    env: Environment<'static>,
    max_depth: usize,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("max_depth", &self.max_depth)
            .finish_non_exhaustive()
    }
}

impl Engine {
    pub fn new() -> Self {
        let mut env = Environment::new();
        env.set_undefined_behavior(UndefinedBehavior::Strict);
        Self {
            env,
            max_depth: DEFAULT_MAX_DEPTH,
        }
    }

    /// Builder form of [`Engine::env_mut`].
    pub fn with_env(mut self, configure: impl FnOnce(&mut Environment<'static>)) -> Self {
        configure(&mut self.env);
        self
    }

    pub fn env(&self) -> &Environment<'static> {
        &self.env
    }

    /// The expression environment, to register functions, filters, tests and globals.
    pub fn env_mut(&mut self) -> &mut Environment<'static> {
        &mut self.env
    }

    pub fn max_depth(&self) -> usize {
        self.max_depth
    }

    /// Sets the limit for nested `use` (default [`DEFAULT_MAX_DEPTH`]).
    pub fn set_max_depth(&mut self, max_depth: usize) {
        self.max_depth = max_depth;
    }

    /// Load-time check without devices: unknown templates, missing and unknown params,
    /// expression syntax, malformed `use`/`for_each`/`when` and template recursion, in the
    /// document and in every template it reaches. The document root is checked like
    /// [`Engine::expand_documents`] treats it (root `for_each` and `when` allowed).
    pub fn check(&self, doc: &Value, registry: &TemplateRegistry) -> Vec<TemplateError> {
        self.check_impl(doc, registry).0
    }

    /// Names of all templates the document uses, directly or through other templates
    /// (including unknown names). Apps re-expand a document when one of them changes.
    pub fn references(&self, doc: &Value, registry: &TemplateRegistry) -> BTreeSet<String> {
        self.check_impl(doc, registry).1
    }

    /// Expands one document. A `for_each` at the root is an error here; use
    /// [`Engine::expand_documents`] for documents that may yield several documents.
    pub fn expand(
        &self,
        doc: &Value,
        registry: &TemplateRegistry,
        ctx: &dyn ExpandContext,
    ) -> Result<Value, TemplateError> {
        let mut expander = Expander::new(self, registry, ctx)?;
        expander.value(doc)
    }

    /// Expands a document whose root is treated as a list item: a root `for_each` yields one
    /// document per element, a false root `when` yields none, a `use` whose body is a list
    /// yields its items; anything else yields exactly one document.
    pub fn expand_documents(
        &self,
        doc: &Value,
        registry: &TemplateRegistry,
        ctx: &dyn ExpandContext,
    ) -> Result<Vec<Value>, TemplateError> {
        let mut expander = Expander::new(self, registry, ctx)?;
        let mut out = Vec::new();
        expander.seq_item(doc, &mut out)?;
        Ok(out)
    }

    fn check_impl(
        &self,
        doc: &Value,
        registry: &TemplateRegistry,
    ) -> (Vec<TemplateError>, BTreeSet<String>) {
        let mut checker = Checker {
            env: &self.env,
            registry,
            path: Vec::new(),
            param: None,
            errors: Vec::new(),
            refs: BTreeSet::new(),
        };
        checker.seq_item(doc);
        let roots = std::mem::take(&mut checker.refs);

        let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut queue: Vec<String> = roots.iter().cloned().collect();
        while let Some(name) = queue.pop() {
            if graph.contains_key(&name) {
                continue;
            }
            let Some(template) = registry.get(&name) else {
                graph.insert(name, BTreeSet::new());
                continue;
            };
            for param in &template.params {
                if let Some(default) = template.default_for(param) {
                    checker.path = vec![
                        PathSegment::Template(name.clone()),
                        PathSegment::Key("defaults".into()),
                        PathSegment::Key(param.clone()),
                    ];
                    checker.param = Some(param.clone());
                    checker.value(default);
                    checker.param = None;
                }
            }
            checker.path = vec![PathSegment::Template(name.clone())];
            checker.body(&template.body);
            let refs = std::mem::take(&mut checker.refs);
            queue.extend(refs.iter().cloned());
            graph.insert(name, refs);
        }

        let mut errors = checker.errors;
        for cycle in find_cycles(&graph, &roots) {
            errors.push(TemplateError::new(
                Kind::Recursion,
                &[PathSegment::Template(cycle[0].clone())],
                format!("template recursion: {}", cycle.join(" -> ")),
            ));
        }
        (errors, graph.into_keys().collect())
    }
}

/// [`Engine::check`] with a default engine.
pub fn check(doc: &Value, registry: &TemplateRegistry) -> Vec<TemplateError> {
    Engine::new().check(doc, registry)
}

/// [`Engine::expand`] with a default engine (no app functions).
pub fn expand(
    doc: &Value,
    registry: &TemplateRegistry,
    ctx: &dyn ExpandContext,
) -> Result<Value, TemplateError> {
    Engine::new().expand(doc, registry, ctx)
}

/// [`Engine::expand_documents`] with a default engine (no app functions).
pub fn expand_documents(
    doc: &Value,
    registry: &TemplateRegistry,
    ctx: &dyn ExpandContext,
) -> Result<Vec<Value>, TemplateError> {
    Engine::new().expand_documents(doc, registry, ctx)
}

// ── expansion ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayerKind {
    Global,
    App,
    Params,
    Item,
}

struct Layer {
    kind: LayerKind,
    vars: Vec<(String, MjValue)>,
    /// Variables visible with this layer on top.
    merged: MjValue,
}

struct Expander<'e> {
    env: &'e Environment<'static>,
    registry: &'e TemplateRegistry,
    ctx: &'e dyn ExpandContext,
    max_depth: usize,
    scope_keys: Vec<String>,
    layers: Vec<Layer>,
    path: Vec<PathSegment>,
    uses: Vec<String>,
    param: Option<String>,
}

const USE_KEYS: &[&str] = &["use", "with"];
const FOR_EACH_USE_KEYS: &[&str] = &["for_each", "use", "with"];
const FOR_EACH_ITEM_KEYS: &[&str] = &["for_each", "item"];
const FOR_EACH_SPEC_KEYS: &[&str] = &["query", "items", "sort_by"];
const FOR_EACH_SPEC_NEEDS: &str = "`for_each` needs exactly one of `query` or `items`";

impl<'e> Expander<'e> {
    fn new(
        engine: &'e Engine,
        registry: &'e TemplateRegistry,
        ctx: &'e dyn ExpandContext,
    ) -> Result<Self, TemplateError> {
        let mut expander = Self {
            env: &engine.env,
            registry,
            ctx,
            max_depth: engine.max_depth,
            scope_keys: ctx.scope_keys(),
            layers: Vec::new(),
            path: Vec::new(),
            uses: Vec::new(),
            param: None,
        };
        let globals = expander.vars(&ctx.globals())?;
        expander.push_layer(LayerKind::Global, globals);
        Ok(expander)
    }

    fn err(&self, kind: Kind, message: impl Into<String>) -> TemplateError {
        TemplateError::new(kind, &self.path, message).param(self.param.as_deref())
    }

    // scope

    fn push_layer(&mut self, kind: LayerKind, vars: Vec<(String, MjValue)>) {
        self.layers.push(Layer {
            kind,
            vars,
            merged: MjValue::UNDEFINED,
        });
        // A template body sees globals, app scopes and everything from its own params on;
        // caller params and `item`s stay outside.
        let boundary = self
            .layers
            .iter()
            .rposition(|l| l.kind == LayerKind::Params)
            .unwrap_or(0);
        let mut merged: BTreeMap<String, MjValue> = BTreeMap::new();
        for (i, layer) in self.layers.iter().enumerate() {
            if i >= boundary || matches!(layer.kind, LayerKind::Global | LayerKind::App) {
                for (k, v) in &layer.vars {
                    merged.insert(k.clone(), v.clone());
                }
            }
        }
        if let Some(top) = self.layers.last_mut() {
            top.merged = MjValue::from(merged);
        }
    }

    fn pop_layer(&mut self) {
        self.layers.pop();
    }

    fn context(&self) -> MjValue {
        self.layers
            .last()
            .map(|l| l.merged.clone())
            .unwrap_or(MjValue::UNDEFINED)
    }

    // conversions

    fn to_mj(&self, value: &Value) -> Result<MjValue, TemplateError> {
        MjValue::deserialize(value.clone()).map_err(|e| self.err(Kind::Conversion, e.to_string()))
    }

    fn to_yaml(&self, value: &MjValue) -> Result<Value, TemplateError> {
        serde_yaml_ng::to_value(value).map_err(|e| self.err(Kind::Conversion, e.to_string()))
    }

    fn vars(&self, mapping: &Mapping) -> Result<Vec<(String, MjValue)>, TemplateError> {
        mapping
            .iter()
            .map(|(k, v)| {
                let key = k
                    .as_str()
                    .ok_or_else(|| self.err(Kind::Conversion, "variable names must be strings"))?;
                Ok((key.to_owned(), self.to_mj(v)?))
            })
            .collect()
    }

    // expressions

    fn eval(&self, expr: &str) -> Result<MjValue, TemplateError> {
        let shown = expr.trim();
        let compiled = self
            .env
            .compile_expression_owned(expr.to_owned())
            .map_err(|e| self.err(Kind::Syntax, format!("`{shown}`: {e}")))?;
        let value = compiled
            .eval(self.context())
            .map_err(|e| self.err(Kind::Evaluation, format!("`{shown}`: {e}")))?;
        if value.is_undefined() {
            return Err(self.err(Kind::Evaluation, format!("`{shown}` is undefined")));
        }
        Ok(value)
    }

    fn scalar(&self, s: &str) -> Result<Value, TemplateError> {
        match parse_scalar(s).map_err(|m| self.err(Kind::Syntax, m))? {
            Scalar::Literal => Ok(Value::String(s.to_owned())),
            Scalar::Single(expr) => self.to_yaml(&self.eval(expr)?),
            Scalar::Mixed(pieces) => {
                let mut out = String::new();
                for piece in pieces {
                    match piece {
                        Piece::Text(text) => out.push_str(text),
                        Piece::Expr(expr) => {
                            let value = self.eval(expr)?;
                            match value.as_str() {
                                Some(s) => out.push_str(s),
                                None => out.push_str(&value.to_string()),
                            }
                        }
                    }
                }
                Ok(Value::String(out))
            }
        }
    }

    fn when(&self, when: &Value) -> Result<bool, TemplateError> {
        match when {
            Value::Bool(b) => Ok(*b),
            Value::String(s) => {
                let expr = when_expression(s).map_err(|m| self.err(Kind::Malformed, m))?;
                Ok(self.eval(expr)?.is_true())
            }
            _ => Err(self.err(
                Kind::Malformed,
                "`when` must be an expression string or a bool",
            )),
        }
    }

    // tree

    fn value(&mut self, value: &Value) -> Result<Value, TemplateError> {
        match value {
            Value::Mapping(m) => self.mapping(m),
            Value::Sequence(items) => {
                let mut out = Vec::with_capacity(items.len());
                for (i, item) in items.iter().enumerate() {
                    self.path.push(PathSegment::Index(i));
                    self.seq_item(item, &mut out)?;
                    self.path.pop();
                }
                Ok(Value::Sequence(out))
            }
            Value::String(s) => self.scalar(s),
            Value::Tagged(tagged) => {
                let inner = self.value(&tagged.value)?;
                Ok(Value::Tagged(Box::new(serde_yaml_ng::value::TaggedValue {
                    tag: tagged.tag.clone(),
                    value: inner,
                })))
            }
            other => Ok(other.clone()),
        }
    }

    fn mapping(&mut self, m: &Mapping) -> Result<Value, TemplateError> {
        if m.contains_key("use") {
            self.check_keys(m, USE_KEYS, false)?;
            let template = self.enter_template(m.get("use"), m.get("with"), None)?;
            let value = self.value(&template.body)?;
            self.exit_template();
            return Ok(value);
        }
        if m.contains_key("for_each") {
            return Err(self.err(
                Kind::Malformed,
                "`for_each` is only allowed as a list item (or at the document root with expand_documents)",
            ));
        }
        self.plain_mapping(m, false)
    }

    fn plain_mapping(&mut self, m: &Mapping, skip_when: bool) -> Result<Value, TemplateError> {
        let mut expanded: HashMap<usize, Value> = HashMap::new();
        let mut scope_entries = Mapping::new();
        if !self.scope_keys.is_empty() {
            for (i, (k, v)) in m.iter().enumerate() {
                let Some(key) = k.as_str() else { continue };
                if (skip_when && key == "when") || !self.scope_keys.iter().any(|s| s == key) {
                    continue;
                }
                self.path.push(PathSegment::Key(key.to_owned()));
                let value = self.value(v)?;
                self.path.pop();
                scope_entries.insert(k.clone(), value.clone());
                expanded.insert(i, value);
            }
        }
        let mut pushed = false;
        if !scope_entries.is_empty() {
            let vars = self
                .ctx
                .scope_vars(&self.path, &scope_entries)
                .map_err(|m| self.err(Kind::Evaluation, m))?;
            if let Some(vars) = vars {
                let vars = self.vars(&vars)?;
                self.push_layer(LayerKind::App, vars);
                pushed = true;
            }
        }
        let mut out = Mapping::with_capacity(m.len());
        for (i, (k, v)) in m.iter().enumerate() {
            if skip_when && k.as_str() == Some("when") {
                continue;
            }
            let value = match expanded.remove(&i) {
                Some(value) => value,
                None => {
                    self.path.push(PathSegment::Key(key_to_string(k)));
                    let value = self.value(v)?;
                    self.path.pop();
                    value
                }
            };
            out.insert(k.clone(), value);
        }
        if pushed {
            self.pop_layer();
        }
        Ok(Value::Mapping(out))
    }

    fn check_keys(
        &self,
        m: &Mapping,
        allowed: &[&str],
        skip_when: bool,
    ) -> Result<(), TemplateError> {
        for key in m.keys() {
            let name = key.as_str();
            if skip_when && name == Some("when") {
                continue;
            }
            if !name.is_some_and(|n| allowed.contains(&n)) {
                return Err(self.err(
                    Kind::Malformed,
                    format!(
                        "unexpected key `{}` next to `{}`",
                        key_to_string(key),
                        allowed[0]
                    ),
                ));
            }
        }
        Ok(())
    }

    /// Appends the expansion of one list item to `out` (zero, one or many values).
    fn seq_item(&mut self, item: &Value, out: &mut Vec<Value>) -> Result<(), TemplateError> {
        let Some(m) = item.as_mapping() else {
            out.push(self.value(item)?);
            return Ok(());
        };
        let mut skip_when = false;
        if let Some(when) = m.get("when") {
            self.path.push(PathSegment::Key("when".into()));
            let keep = self.when(when)?;
            self.path.pop();
            if !keep {
                return Ok(());
            }
            skip_when = true;
        }
        if m.contains_key("for_each") {
            self.for_each(m, skip_when, out)
        } else if m.contains_key("use") {
            self.check_keys(m, USE_KEYS, skip_when)?;
            let template = self.enter_template(m.get("use"), m.get("with"), None)?;
            self.body_into(&template.body, out)?;
            self.exit_template();
            Ok(())
        } else {
            out.push(self.plain_mapping(m, skip_when)?);
            Ok(())
        }
    }

    fn body_into(&mut self, body: &Value, out: &mut Vec<Value>) -> Result<(), TemplateError> {
        match body {
            Value::Sequence(items) => {
                for (i, item) in items.iter().enumerate() {
                    self.path.push(PathSegment::Index(i));
                    self.seq_item(item, out)?;
                    self.path.pop();
                }
                Ok(())
            }
            other => self.seq_item(other, out),
        }
    }

    fn enter_template(
        &mut self,
        name: Option<&Value>,
        with: Option<&Value>,
        auto_item: Option<&MjValue>,
    ) -> Result<&'e Template, TemplateError> {
        let name = name
            .and_then(Value::as_str)
            .ok_or_else(|| self.err(Kind::Malformed, "`use` must be a template name"))?;
        let template = self
            .registry
            .get(name)
            .ok_or_else(|| self.err(Kind::UnknownTemplate, format!("unknown template `{name}`")))?;
        if self.uses.iter().any(|u| u == name) {
            return Err(self.err(
                Kind::Recursion,
                format!("template recursion: {} -> {name}", self.uses.join(" -> ")),
            ));
        }
        if self.uses.len() >= self.max_depth {
            return Err(self.err(
                Kind::DepthLimit,
                format!("nested `use` deeper than {} (at `{name}`)", self.max_depth),
            ));
        }

        let mut given: Vec<(String, MjValue)> = Vec::new();
        if let Some(with) = with {
            let with = with
                .as_mapping()
                .ok_or_else(|| self.err(Kind::Malformed, "`with` must be a mapping"))?;
            for (k, v) in with {
                let key = k
                    .as_str()
                    .ok_or_else(|| self.err(Kind::Malformed, "parameter names must be strings"))?;
                if !template.params.iter().any(|p| p == key) {
                    return Err(self
                        .err(
                            Kind::UnknownParam,
                            format!("template `{name}` has no parameter `{key}`"),
                        )
                        .param(Some(key)));
                }
                self.path.push(PathSegment::Key("with".into()));
                self.path.push(PathSegment::Key(key.to_owned()));
                let previous = self.param.replace(key.to_owned());
                let value = self.value(v)?;
                let value = self.to_mj(&value)?;
                self.param = previous;
                self.path.truncate(self.path.len() - 2);
                given.push((key.to_owned(), value));
            }
        }
        if let Some(item) = auto_item {
            if template.params.iter().any(|p| p == "item")
                && !given.iter().any(|(k, _)| k == "item")
            {
                given.push(("item".to_owned(), item.clone()));
            }
        }
        for param in &template.params {
            if !given.iter().any(|(k, _)| k == param) && template.default_for(param).is_none() {
                return Err(self
                    .err(
                        Kind::MissingParam,
                        format!("missing parameter `{param}` for template `{name}`"),
                    )
                    .param(Some(param)));
            }
        }

        self.path.push(PathSegment::Template(name.to_owned()));
        self.uses.push(name.to_owned());
        self.push_layer(LayerKind::Params, given.clone());
        let previous = self.param.take();
        for param in &template.params {
            if given.iter().any(|(k, _)| k == param) {
                continue;
            }
            let Some(default) = template.default_for(param) else {
                continue;
            };
            self.path.push(PathSegment::Key("defaults".into()));
            self.path.push(PathSegment::Key(param.clone()));
            self.param = Some(param.clone());
            let value = self.value(default)?;
            let value = self.to_mj(&value)?;
            self.path.truncate(self.path.len() - 2);
            given.push((param.clone(), value));
            self.pop_layer();
            self.push_layer(LayerKind::Params, given.clone());
        }
        self.param = previous;
        Ok(template)
    }

    fn exit_template(&mut self) {
        self.pop_layer();
        self.uses.pop();
        self.path.pop();
    }

    /// The elements of `for_each.items`: a literal list (expanded in the current scope like any
    /// other value) or one `{{ expr }}` that evaluates to a list.
    fn list_items(&mut self, items: &Value) -> Result<Vec<Value>, TemplateError> {
        let expanded = match items {
            Value::Sequence(_) => self.value(items)?,
            Value::String(s) => match parse_scalar(s).map_err(|m| self.err(Kind::Syntax, m))? {
                Scalar::Single(expr) => self.to_yaml(&self.eval(expr)?)?,
                _ => {
                    return Err(self.err(
                        Kind::Malformed,
                        "`items` must be a list or a single `{{ expr }}` returning a list",
                    ))
                }
            },
            _ => {
                return Err(self.err(
                    Kind::Malformed,
                    "`items` must be a list or a single `{{ expr }}` returning a list",
                ))
            }
        };
        match expanded {
            Value::Sequence(items) => Ok(items),
            Value::Null => Err(self.err(Kind::Evaluation, "`items` evaluated to none, not a list")),
            other => Err(self.err(
                Kind::Evaluation,
                format!(
                    "`items` must evaluate to a list, got {}",
                    serde_yaml_ng::to_string(&other)
                        .map(|s| s.trim().to_owned())
                        .unwrap_or_default()
                ),
            )),
        }
    }

    fn for_each(
        &mut self,
        m: &Mapping,
        skip_when: bool,
        out: &mut Vec<Value>,
    ) -> Result<(), TemplateError> {
        let use_name = m.get("use");
        let inline = m.get("item");
        if use_name.is_some() == inline.is_some() {
            return Err(self.err(
                Kind::Malformed,
                "`for_each` needs exactly one of `use` or `item`",
            ));
        }
        let allowed = if use_name.is_some() {
            FOR_EACH_USE_KEYS
        } else {
            FOR_EACH_ITEM_KEYS
        };
        self.check_keys(m, allowed, skip_when)?;

        self.path.push(PathSegment::Key("for_each".into()));
        let spec = m
            .get("for_each")
            .and_then(Value::as_mapping)
            .ok_or_else(|| {
                self.err(
                    Kind::Malformed,
                    "`for_each` must be a mapping with `query` or `items`",
                )
            })?;
        self.check_keys(spec, FOR_EACH_SPEC_KEYS, false)?;
        let sort_by = match spec.get("sort_by") {
            None => None,
            Some(Value::String(s)) => Some(s.as_str()),
            Some(_) => {
                return Err(self.err(Kind::Malformed, "`sort_by` must be a field path string"))
            }
        };
        let items = match (spec.get("query"), spec.get("items")) {
            (Some(query), None) => {
                self.path.push(PathSegment::Key("query".into()));
                let query = self.value(query)?;
                let query: QueryDefinition = serde_yaml_ng::from_value(query)
                    .map_err(|e| self.err(Kind::Malformed, format!("invalid query: {e}")))?;
                self.path.pop();
                let mut items = self
                    .ctx
                    .query(&query)
                    .map_err(|e| self.err(Kind::Query, e))?;
                sort_query_elements(&mut items, sort_by);
                items
            }
            (None, Some(items)) => {
                self.path.push(PathSegment::Key("items".into()));
                let mut items = self.list_items(items)?;
                self.path.pop();
                if let Some(sort_by) = sort_by {
                    sort_list_elements(&mut items, sort_by);
                }
                items
            }
            _ => return Err(self.err(Kind::Malformed, FOR_EACH_SPEC_NEEDS)),
        };
        self.path.pop();

        for (index, element) in items.iter().enumerate() {
            self.path
                .push(PathSegment::Element(element_label(element, index)));
            let item = self.to_mj(element)?;
            self.push_layer(LayerKind::Item, vec![("item".to_owned(), item.clone())]);
            if let Some(inline) = inline {
                self.path.push(PathSegment::Key("item".into()));
                self.seq_item(inline, out)?;
                self.path.pop();
            } else {
                let template = self.enter_template(use_name, m.get("with"), Some(&item))?;
                self.body_into(&template.body, out)?;
                self.exit_template();
            }
            self.pop_layer();
            self.path.pop();
        }
        Ok(())
    }
}

// ── for_each ordering ────────────────────────────────────────────────────

fn field<'v>(value: &'v Value, path: &str) -> Option<&'v Value> {
    let mut current = value;
    for part in path.split('.') {
        current = current.as_mapping()?.get(part)?;
    }
    let current = match current {
        Value::Sequence(items) => items.first()?,
        other => other,
    };
    (!current.is_null()).then_some(current)
}

fn rank(value: &Value) -> u8 {
    match value {
        Value::Bool(_) => 0,
        Value::Number(_) => 1,
        Value::String(_) => 2,
        _ => 3,
    }
}

fn compare_values(a: &Value, b: &Value) -> Ordering {
    match (a, b) {
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (
                x.as_f64().unwrap_or(f64::NAN),
                y.as_f64().unwrap_or(f64::NAN),
            );
            x.partial_cmp(&y).unwrap_or(Ordering::Equal)
        }
        (Value::String(x), Value::String(y)) => x.cmp(y),
        _ => rank(a).cmp(&rank(b)),
    }
}

/// Missing values sort last.
fn compare_optional(a: Option<&Value>, b: Option<&Value>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => compare_values(a, b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// Query results: by `sort_by`, then by `id` (store order is not deterministic).
fn sort_query_elements(items: &mut [Value], sort_by: Option<&str>) {
    items.sort_by(|a, b| {
        let by_field = match sort_by {
            Some(path) => compare_optional(field(a, path), field(b, path)),
            None => Ordering::Equal,
        };
        by_field.then_with(|| compare_optional(field(a, "id"), field(b, "id")))
    });
}

/// `items` lists: by `sort_by`, ties keep list order (stable sort).
fn sort_list_elements(items: &mut [Value], sort_by: &str) {
    items.sort_by(|a, b| compare_optional(field(a, sort_by), field(b, sort_by)));
}

/// Path label of a `for_each` element: its `id` field, a scalar element itself, or its index.
fn element_label(element: &Value, index: usize) -> String {
    let label = match element {
        Value::Mapping(m) => m.get("id"),
        scalar => Some(scalar),
    };
    match label {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        _ => index.to_string(),
    }
}

// ── check ────────────────────────────────────────────────────────────────

struct Checker<'e> {
    env: &'e Environment<'static>,
    registry: &'e TemplateRegistry,
    path: Vec<PathSegment>,
    param: Option<String>,
    errors: Vec<TemplateError>,
    /// Templates used by the unit (document or template body) being walked.
    refs: BTreeSet<String>,
}

impl Checker<'_> {
    fn report(&mut self, kind: Kind, message: impl Into<String>) {
        let error = TemplateError::new(kind, &self.path, message).param(self.param.as_deref());
        self.errors.push(error);
    }

    fn with_key(&mut self, key: &str, f: impl FnOnce(&mut Self)) {
        self.path.push(PathSegment::Key(key.to_owned()));
        f(self);
        self.path.pop();
    }

    fn expr(&mut self, expr: &str) {
        if let Err(e) = self.env.compile_expression_owned(expr.to_owned()) {
            self.report(Kind::Syntax, format!("`{}`: {e}", expr.trim()));
        }
    }

    fn scalar(&mut self, s: &str) {
        match parse_scalar(s) {
            Err(m) => self.report(Kind::Syntax, m),
            Ok(Scalar::Literal) => {}
            Ok(Scalar::Single(expr)) => self.expr(expr),
            Ok(Scalar::Mixed(pieces)) => {
                for piece in pieces {
                    if let Piece::Expr(expr) = piece {
                        self.expr(expr);
                    }
                }
            }
        }
    }

    fn value(&mut self, value: &Value) {
        match value {
            Value::Mapping(m) => self.mapping(m),
            Value::Sequence(items) => {
                for (i, item) in items.iter().enumerate() {
                    self.path.push(PathSegment::Index(i));
                    self.seq_item(item);
                    self.path.pop();
                }
            }
            Value::String(s) => self.scalar(s),
            Value::Tagged(tagged) => self.value(&tagged.value),
            _ => {}
        }
    }

    fn body(&mut self, body: &Value) {
        match body {
            Value::Sequence(_) => self.value(body),
            other => self.seq_item(other),
        }
    }

    fn mapping(&mut self, m: &Mapping) {
        if m.contains_key("use") {
            self.keys(m, USE_KEYS, false);
            self.use_site(m.get("use"), m.get("with"), false);
        } else if m.contains_key("for_each") {
            self.report(
                Kind::Malformed,
                "`for_each` is only allowed as a list item or at the document root",
            );
        } else {
            self.plain(m, false);
        }
    }

    fn plain(&mut self, m: &Mapping, skip_when: bool) {
        for (k, v) in m {
            if skip_when && k.as_str() == Some("when") {
                continue;
            }
            self.path.push(PathSegment::Key(key_to_string(k)));
            self.value(v);
            self.path.pop();
        }
    }

    fn keys(&mut self, m: &Mapping, allowed: &[&str], skip_when: bool) {
        for key in m.keys() {
            let name = key.as_str();
            if skip_when && name == Some("when") {
                continue;
            }
            if !name.is_some_and(|n| allowed.contains(&n)) {
                self.report(
                    Kind::Malformed,
                    format!(
                        "unexpected key `{}` next to `{}`",
                        key_to_string(key),
                        allowed[0]
                    ),
                );
            }
        }
    }

    fn seq_item(&mut self, item: &Value) {
        let Some(m) = item.as_mapping() else {
            self.value(item);
            return;
        };
        let mut skip_when = false;
        if let Some(when) = m.get("when") {
            skip_when = true;
            self.with_key("when", |c| match when {
                Value::Bool(_) => {}
                Value::String(s) => match when_expression(s) {
                    Ok(expr) => c.expr(expr),
                    Err(m) => c.report(Kind::Malformed, m),
                },
                _ => c.report(
                    Kind::Malformed,
                    "`when` must be an expression string or a bool",
                ),
            });
        }
        if m.contains_key("for_each") {
            self.for_each(m, skip_when);
        } else if m.contains_key("use") {
            self.keys(m, USE_KEYS, skip_when);
            self.use_site(m.get("use"), m.get("with"), false);
        } else {
            self.plain(m, skip_when);
        }
    }

    fn use_site(&mut self, name: Option<&Value>, with: Option<&Value>, auto_item: bool) {
        let Some(name) = name.and_then(Value::as_str) else {
            self.report(Kind::Malformed, "`use` must be a template name");
            return;
        };
        self.refs.insert(name.to_owned());
        let template = self.registry.get(name);
        if template.is_none() {
            self.report(Kind::UnknownTemplate, format!("unknown template `{name}`"));
        }
        let mut given = BTreeSet::new();
        match with {
            None => {}
            Some(Value::Mapping(with)) => {
                for (k, v) in with {
                    let Some(key) = k.as_str() else {
                        self.report(Kind::Malformed, "parameter names must be strings");
                        continue;
                    };
                    given.insert(key.to_owned());
                    if let Some(t) = template {
                        if !t.params.iter().any(|p| p == key) {
                            let error = TemplateError::new(
                                Kind::UnknownParam,
                                &self.path,
                                format!("template `{name}` has no parameter `{key}`"),
                            )
                            .param(Some(key));
                            self.errors.push(error);
                        }
                    }
                    self.path.push(PathSegment::Key("with".into()));
                    self.path.push(PathSegment::Key(key.to_owned()));
                    let previous = self.param.replace(key.to_owned());
                    self.value(v);
                    self.param = previous;
                    self.path.truncate(self.path.len() - 2);
                }
            }
            Some(_) => self.report(Kind::Malformed, "`with` must be a mapping"),
        }
        if let Some(t) = template {
            for param in &t.params {
                let supplied = given.contains(param)
                    || t.default_for(param).is_some()
                    || (auto_item && param == "item");
                if !supplied {
                    let error = TemplateError::new(
                        Kind::MissingParam,
                        &self.path,
                        format!("missing parameter `{param}` for template `{name}`"),
                    )
                    .param(Some(param));
                    self.errors.push(error);
                }
            }
        }
    }

    fn for_each(&mut self, m: &Mapping, skip_when: bool) {
        let use_name = m.get("use");
        let inline = m.get("item");
        if use_name.is_some() == inline.is_some() {
            self.report(
                Kind::Malformed,
                "`for_each` needs exactly one of `use` or `item`",
            );
        }
        let allowed = if use_name.is_some() {
            FOR_EACH_USE_KEYS
        } else {
            FOR_EACH_ITEM_KEYS
        };
        self.keys(m, allowed, skip_when);
        self.with_key("for_each", |c| {
            let Some(spec) = m.get("for_each").and_then(Value::as_mapping) else {
                c.report(
                    Kind::Malformed,
                    "`for_each` must be a mapping with `query` or `items`",
                );
                return;
            };
            c.keys(spec, FOR_EACH_SPEC_KEYS, false);
            match (spec.get("query"), spec.get("items")) {
                (Some(query), None) => c.with_key("query", |c| {
                    if has_placeholder(query) {
                        c.value(query);
                    } else if let Err(e) =
                        serde_yaml_ng::from_value::<QueryDefinition>(query.clone())
                    {
                        c.report(Kind::Malformed, format!("invalid query: {e}"));
                    }
                }),
                (None, Some(items)) => c.with_key("items", |c| match items {
                    Value::Sequence(_) => c.value(items),
                    Value::String(s) => match parse_scalar(s) {
                        Err(m) => c.report(Kind::Syntax, m),
                        Ok(Scalar::Single(expr)) => c.expr(expr),
                        Ok(_) => c.report(
                            Kind::Malformed,
                            "`items` must be a list or a single `{{ expr }}` returning a list",
                        ),
                    },
                    _ => c.report(
                        Kind::Malformed,
                        "`items` must be a list or a single `{{ expr }}` returning a list",
                    ),
                }),
                _ => c.report(Kind::Malformed, FOR_EACH_SPEC_NEEDS),
            }
            if spec.get("sort_by").is_some_and(|s| !s.is_string()) {
                c.report(Kind::Malformed, "`sort_by` must be a field path string");
            }
        });
        if let Some(inline) = inline {
            self.with_key("item", |c| c.seq_item(inline));
        } else if use_name.is_some() {
            self.use_site(use_name, m.get("with"), true);
        }
    }
}

fn has_placeholder(value: &Value) -> bool {
    match value {
        Value::String(s) => s.contains("{{"),
        Value::Sequence(items) => items.iter().any(has_placeholder),
        Value::Mapping(m) => m.values().any(has_placeholder),
        Value::Tagged(t) => has_placeholder(&t.value),
        _ => false,
    }
}

/// Cycles reachable from `roots`, each reported once (`a -> b -> a`).
fn find_cycles(
    graph: &BTreeMap<String, BTreeSet<String>>,
    roots: &BTreeSet<String>,
) -> Vec<Vec<String>> {
    fn visit(
        node: &str,
        graph: &BTreeMap<String, BTreeSet<String>>,
        stack: &mut Vec<String>,
        done: &mut BTreeSet<String>,
        seen_cycles: &mut BTreeSet<BTreeSet<String>>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        if let Some(pos) = stack.iter().position(|n| n == node) {
            let mut cycle: Vec<String> = stack[pos..].to_vec();
            if seen_cycles.insert(cycle.iter().cloned().collect()) {
                cycle.push(node.to_owned());
                cycles.push(cycle);
            }
            return;
        }
        if done.contains(node) {
            return;
        }
        stack.push(node.to_owned());
        if let Some(next) = graph.get(node) {
            for n in next {
                visit(n, graph, stack, done, seen_cycles, cycles);
            }
        }
        stack.pop();
        done.insert(node.to_owned());
    }

    let mut cycles = Vec::new();
    let mut done = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for root in roots {
        visit(
            root,
            graph,
            &mut Vec::new(),
            &mut done,
            &mut seen,
            &mut cycles,
        );
    }
    cycles
}
