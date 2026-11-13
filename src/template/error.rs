use std::fmt;

use serde_yaml_ng::Value;

/// One step of a [`YamlPath`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathSegment {
    /// Mapping key.
    Key(String),
    /// Sequence index in the source tree (before splicing).
    Index(usize),
    /// Entry into the body (or defaults) of the named template.
    Template(String),
    /// One element of a `for_each`: its `id` field, or its position when it has none.
    Element(String),
}

/// Location of a node in a document, across template boundaries.
///
/// Rendered like `cards[3].items[1] > heating-room-card > items[0].label`: the part before the
/// first `>` is the document, each `> name >` enters the body of template `name` (its defaults
/// appear as `defaults.<param>`), and `[item=<id>]` marks one element of a `for_each`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct YamlPath(pub Vec<PathSegment>);

impl YamlPath {
    pub fn segments(&self) -> &[PathSegment] {
        &self.0
    }
}

impl fmt::Display for YamlPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return f.write_str("(root)");
        }
        let mut first = true;
        let mut after_template = false;
        for segment in &self.0 {
            match segment {
                PathSegment::Template(name) => {
                    if first {
                        write!(f, "{name}")?;
                    } else {
                        write!(f, " > {name}")?;
                    }
                    after_template = true;
                }
                other => {
                    if after_template {
                        f.write_str(" > ")?;
                    }
                    match other {
                        PathSegment::Key(key) if first || after_template => write!(f, "{key}")?,
                        PathSegment::Key(key) => write!(f, ".{key}")?,
                        PathSegment::Index(index) => write!(f, "[{index}]")?,
                        PathSegment::Element(id) => write!(f, "[item={id}]")?,
                        PathSegment::Template(_) => unreachable!(),
                    }
                    after_template = false;
                }
            }
            first = false;
        }
        Ok(())
    }
}

/// What went wrong; see [`TemplateError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateErrorKind {
    /// A template document is malformed (missing `body`, duplicate params, unknown default).
    InvalidTemplate,
    /// `use:` names a template that is not registered.
    UnknownTemplate,
    /// A declared parameter has neither a `with` value nor a default.
    MissingParam,
    /// `with` passes a parameter the template does not declare.
    UnknownParam,
    /// `use`, `for_each` or `when` has the wrong shape or extra keys.
    Malformed,
    /// An expression does not parse.
    Syntax,
    /// An expression failed or produced an undefined value.
    Evaluation,
    /// A template uses itself, directly or through other templates.
    Recursion,
    /// Nested `use` exceeded the engine's depth limit.
    DepthLimit,
    /// The context failed to evaluate a `for_each` query.
    Query,
    /// A value could not be converted between YAML and the expression engine.
    Conversion,
}

/// An error found while checking or expanding a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateError {
    pub kind: TemplateErrorKind,
    /// Document name; the library never knows it, apps set it with [`TemplateError::with_document`].
    pub document: Option<String>,
    /// Innermost template whose body or defaults contain the error, if any.
    pub template: Option<String>,
    /// Parameter whose value (`with` or default) contains the error, if any.
    pub param: Option<String>,
    pub path: YamlPath,
    pub message: String,
}

impl TemplateError {
    pub(crate) fn new(
        kind: TemplateErrorKind,
        path: &[PathSegment],
        message: impl Into<String>,
    ) -> Self {
        let template = path.iter().rev().find_map(|s| match s {
            PathSegment::Template(name) => Some(name.clone()),
            _ => None,
        });
        Self {
            kind,
            document: None,
            template,
            param: None,
            path: YamlPath(path.to_vec()),
            message: message.into(),
        }
    }

    pub(crate) fn param(mut self, param: Option<&str>) -> Self {
        if self.param.is_none() {
            self.param = param.map(str::to_owned);
        }
        self
    }

    /// Sets the document name shown in the message.
    pub fn with_document(mut self, document: impl Into<String>) -> Self {
        self.document = Some(document.into());
        self
    }
}

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(document) = &self.document {
            write!(f, "{document}: ")?;
        }
        write!(f, "at {}", self.path)?;
        match (&self.template, &self.param) {
            (Some(t), Some(p)) => write!(f, " (template `{t}`, param `{p}`)")?,
            (Some(t), None) => write!(f, " (template `{t}`)")?,
            (None, Some(p)) => write!(f, " (param `{p}`)")?,
            (None, None) => {}
        }
        write!(f, ": {}", self.message)
    }
}

impl std::error::Error for TemplateError {}

pub(crate) fn key_to_string(key: &Value) -> String {
    match key {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "~".to_owned(),
        other => serde_yaml_ng::to_string(other)
            .map(|s| s.trim().to_owned())
            .unwrap_or_else(|_| "?".to_owned()),
    }
}
