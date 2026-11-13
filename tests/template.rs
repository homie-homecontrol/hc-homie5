#![cfg(feature = "template")]

use std::cell::RefCell;

use hc_homie5::query::QueryDefinition;
use hc_homie5::template::minijinja::{Error as MjError, State, Value as MjValue};
use hc_homie5::template::{
    check, expand, expand_documents, EmptyContext, Engine, ExpandContext, PathSegment, Template,
    TemplateErrorKind, TemplateRegistry,
};
use serde_yaml_ng::{Mapping, Value};

fn yaml(s: &str) -> Value {
    serde_yaml_ng::from_str(s).expect("valid yaml")
}

fn registry(templates: &[&str]) -> TemplateRegistry {
    let mut registry = TemplateRegistry::new();
    for t in templates {
        registry.insert_document(&yaml(t)).expect("valid template");
    }
    registry
}

/// Devices as owned YAML snapshots; `query` filters by `node.type` and records the type it saw.
struct Fake {
    devices: Vec<Value>,
    seen_node_types: RefCell<Vec<String>>,
    globals: Mapping,
}

impl Fake {
    fn new(devices: &str) -> Self {
        let devices = match yaml(devices) {
            Value::Sequence(items) => items,
            _ => panic!("devices must be a list"),
        };
        Self {
            devices,
            seen_node_types: RefCell::new(Vec::new()),
            globals: Mapping::new(),
        }
    }

    fn device(&self, id: &str) -> Option<&Value> {
        self.devices
            .iter()
            .find(|d| d.get("id").and_then(Value::as_str) == Some(id))
    }
}

impl ExpandContext for Fake {
    fn query(&self, query: &QueryDefinition) -> Result<Vec<Value>, String> {
        let node_type = query
            .node
            .as_ref()
            .and_then(|n| n.r#type.as_ref())
            .and_then(|c| c.value())
            .cloned();
        if let Some(t) = &node_type {
            self.seen_node_types.borrow_mut().push(t.clone());
            if t == "fail" {
                return Err("store unavailable".into());
            }
        }
        Ok(self
            .devices
            .iter()
            .filter(|d| match &node_type {
                None => true,
                Some(t) => d
                    .get("node_types")
                    .and_then(Value::as_sequence)
                    .is_some_and(|types| types.iter().any(|x| x.as_str() == Some(t))),
            })
            .cloned()
            .collect())
    }

    fn globals(&self) -> Mapping {
        self.globals.clone()
    }

    fn scope_keys(&self) -> Vec<String> {
        vec!["device".into()]
    }

    fn scope_vars(&self, _path: &[PathSegment], keys: &Mapping) -> Result<Option<Mapping>, String> {
        let Some(id) = keys.get("device").and_then(Value::as_str) else {
            return Ok(None);
        };
        let Some(device) = self.device(id) else {
            return Ok(None);
        };
        let mut vars = Mapping::new();
        vars.insert("device".into(), device.clone());
        Ok(Some(vars))
    }
}

const DEVICES: &str = r#"
- id: thermo-b
  name: Bath thermostat
  meta: { room: [Bath, Upstairs], heating_room: true }
  node_types: [thermostat]
- id: thermo-a
  name: Office thermostat
  meta: { room: [Office], heating_room: true }
  node_types: [thermostat]
- id: thermo-c
  name: Kitchen thermostat
  meta: { room: Bath }
  node_types: [thermostat]
- id: lamp-1
  name: Lamp
  node_types: [switch]
"#;

#[test]
fn placeholder_only_strings_yield_typed_values() {
    let reg = registry(&[r#"
template: typed
params: [n, l, m, flag, s]
body:
  number: "{{ n }}"
  sum: "  {{ n + 1 }} "
  list: "{{ l }}"
  map: "{{ m }}"
  flag: "{{ flag }}"
  string: "{{ s }}"
  none: "{{ none }}"
  literal: plain {% raw %}
"#]);
    let doc =
        yaml("{ use: typed, with: { n: 41, l: [1, two], m: { b: 1, a: 2 }, flag: true, s: '7' } }");
    let out = expand(&doc, &reg, &EmptyContext).unwrap();
    assert_eq!(
        out,
        yaml(
            r#"
number: 41
sum: 42
list: [1, two]
map: { b: 1, a: 2 }
flag: true
string: "7"
none: null
literal: plain {% raw %}
"#
        )
    );
}

#[test]
fn mixed_text_renders_a_string() {
    let reg = registry(&["{ template: t, params: [room, n, l], body: { label: 'Room {{ room }} ({{ n }}) {{ l }}' } }"]);
    let doc = yaml("{ use: t, with: { room: Office, n: 2, l: [1, 2] } }");
    assert_eq!(
        expand(&doc, &reg, &EmptyContext).unwrap(),
        yaml("{ label: 'Room Office (2) [1, 2]' }")
    );
}

#[test]
fn defaults_apply_and_may_use_params() {
    let reg = registry(&[r#"
template: t
params: [room, title, size]
defaults: { title: "{{ room }} heating", size: 2 }
body: { title: "{{ title }}", size: "{{ size }}" }
"#]);
    let out = expand(
        &yaml("{ use: t, with: { room: Bath } }"),
        &reg,
        &EmptyContext,
    )
    .unwrap();
    assert_eq!(out, yaml("{ title: Bath heating, size: 2 }"));
    let out = expand(
        &yaml("{ use: t, with: { room: Bath, size: 4 } }"),
        &reg,
        &EmptyContext,
    )
    .unwrap();
    assert_eq!(out, yaml("{ title: Bath heating, size: 4 }"));
}

#[test]
fn list_bodies_splice_into_lists() {
    let reg =
        registry(&["{ template: pair, params: [x], body: [{ a: '{{ x }}' }, { b: '{{ x }}' }] }"]);
    let doc = yaml("items: [first, { use: pair, with: { x: 1 } }, last]");
    assert_eq!(
        expand(&doc, &reg, &EmptyContext).unwrap(),
        yaml("items: [first, { a: 1 }, { b: 1 }, last]")
    );
    // Outside a list the list body is the value.
    let doc = yaml("items: { use: pair, with: { x: 2 } }");
    assert_eq!(
        expand(&doc, &reg, &EmptyContext).unwrap(),
        yaml("items: [{ a: 2 }, { b: 2 }]")
    );
}

#[test]
fn nested_use_passes_params_explicitly() {
    let reg = registry(&[
        "{ template: inner, params: [label], body: { text: '{{ label }}' } }",
        "{ template: outer, params: [room], body: { title: '{{ room }}', items: [{ use: inner, with: { label: '{{ room }} temp' } }] } }",
        "{ template: leaky, params: [room], body: { use: peek } }",
        "{ template: peek, body: { text: '{{ room }}' } }",
    ]);
    let out = expand(
        &yaml("{ use: outer, with: { room: Bath } }"),
        &reg,
        &EmptyContext,
    )
    .unwrap();
    assert_eq!(out, yaml("{ title: Bath, items: [{ text: Bath temp }] }"));

    // Caller params are not visible in a used template's body.
    let err = expand(
        &yaml("{ use: leaky, with: { room: Bath } }"),
        &reg,
        &EmptyContext,
    )
    .unwrap_err();
    assert_eq!(err.kind, TemplateErrorKind::Evaluation);
    assert_eq!(err.template.as_deref(), Some("peek"));
    assert_eq!(err.path.to_string(), "leaky > peek > text");
}

#[test]
fn when_keeps_or_drops_list_items() {
    let reg = registry(&["{ template: w, params: [x], body: { w: '{{ x }}' } }"]);
    let doc = yaml(
        r#"
items:
  - { when: "show", a: 1 }
  - { when: "not show", b: 2 }
  - { when: "{{ n > 1 }}", c: 3 }
  - { when: false, d: 4 }
  - { when: "show", use: w, with: { x: 5 } }
"#,
    );
    let mut ctx = Fake::new("[]");
    ctx.globals.insert("show".into(), true.into());
    ctx.globals.insert("n".into(), 2.into());
    assert_eq!(
        expand(&doc, &reg, &ctx).unwrap(),
        yaml("items: [{ a: 1 }, { c: 3 }, { w: 5 }]")
    );
}

#[test]
fn for_each_with_use_and_inline_item() {
    let reg = registry(&[
        "{ template: card, params: [item, title], defaults: { title: '{{ item.name }}' }, body: { card: '{{ item.id }}', title: '{{ title }}' } }",
        "{ template: devices-of, params: [kind], body: [{ for_each: { query: { node: { type: '{{ kind }}' } } }, item: { id: '{{ item.id }}' } }] }",
    ]);
    let ctx = Fake::new(DEVICES);

    let doc = yaml(
        r#"
cards:
  - head
  - for_each: { query: { node: { type: thermostat } } }
    use: card
  - for_each: { query: { node: { type: switch } } }
    use: card
    with: { title: "Switch {{ item.name }}" }
  - for_each: { query: { node: { type: thermostat } }, sort_by: name }
    item: { name: "{{ item.name }}", room: "{{ item.meta.room }}" }
"#,
    );
    let expected = yaml(
        r#"
cards:
  - head
  - { card: thermo-a, title: Office thermostat }
  - { card: thermo-b, title: Bath thermostat }
  - { card: thermo-c, title: Kitchen thermostat }
  - { card: lamp-1, title: Switch Lamp }
  - { name: Bath thermostat, room: [Bath, Upstairs] }
  - { name: Kitchen thermostat, room: Bath }
  - { name: Office thermostat, room: [Office] }
"#,
    );
    assert_eq!(expand(&doc, &reg, &ctx).unwrap(), expected);

    // Queries are expanded with the template's params before evaluation.
    let out = expand(
        &yaml("{ use: devices-of, with: { kind: switch } }"),
        &reg,
        &ctx,
    )
    .unwrap();
    assert_eq!(out, yaml("[{ id: lamp-1 }]"));
    assert!(ctx.seen_node_types.borrow().contains(&"switch".to_owned()));
}

#[test]
fn sort_by_uses_first_list_element_then_id_and_missing_last() {
    let ctx = Fake::new(DEVICES);
    let doc = yaml("- { for_each: { query: {}, sort_by: meta.room }, item: '{{ item.id }}' }");
    // Bath (thermo-b list first element, thermo-c scalar) tie-break by id, then Office, then lamp-1 without meta.
    assert_eq!(
        expand(&doc, &TemplateRegistry::new(), &ctx).unwrap(),
        yaml("[thermo-b, thermo-c, thermo-a, lamp-1]")
    );
    // Without sort_by the order is by id.
    let doc = yaml("- { for_each: { query: {} }, item: '{{ item.id }}' }");
    assert_eq!(
        expand(&doc, &TemplateRegistry::new(), &ctx).unwrap(),
        yaml("[lamp-1, thermo-a, thermo-b, thermo-c]")
    );
}

#[test]
fn root_for_each_yields_documents() {
    let ctx = Fake::new(DEVICES);
    let doc = yaml(
        r#"
for_each: { query: { node: { type: thermostat } }, sort_by: name }
item: { id: "virtual-{{ item.id }}", name: "{{ item.name }}" }
"#,
    );
    let docs = expand_documents(&doc, &TemplateRegistry::new(), &ctx).unwrap();
    let ids: Vec<&str> = docs.iter().map(|d| d["id"].as_str().unwrap()).collect();
    assert_eq!(
        ids,
        ["virtual-thermo-b", "virtual-thermo-c", "virtual-thermo-a"]
    );

    // A plain document yields itself; expand() rejects a root for_each.
    assert_eq!(
        expand_documents(&yaml("{ a: 1 }"), &TemplateRegistry::new(), &ctx).unwrap(),
        vec![yaml("{ a: 1 }")]
    );
    let err = expand(&doc, &TemplateRegistry::new(), &ctx).unwrap_err();
    assert_eq!(err.kind, TemplateErrorKind::Malformed);
}

#[test]
fn strict_undefined_reports_path_template_and_param() {
    let reg = registry(&[
        "{ template: card, params: [room], body: { items: [{ a: 1 }, { label: '{{ rooom }}' }] } }",
        "{ template: p, params: [x], body: '{{ x }}' }",
    ]);
    let doc = yaml("cards: [{ x: 1 }, { x: 2 }, { x: 3 }, { use: card, with: { room: Bath } }]");
    let err = expand(&doc, &reg, &EmptyContext).unwrap_err();
    assert_eq!(err.kind, TemplateErrorKind::Evaluation);
    assert_eq!(err.template.as_deref(), Some("card"));
    assert_eq!(err.path.to_string(), "cards[3] > card > items[1].label");
    assert!(err.message.contains("rooom"), "{}", err.message);

    let err = expand(
        &yaml("{ use: p, with: { x: '{{ missing.attr }}' } }"),
        &reg,
        &EmptyContext,
    )
    .unwrap_err();
    assert_eq!(err.param.as_deref(), Some("x"));
    assert_eq!(err.path.to_string(), "with.x");
    let err = err.with_document("pages/heating.yaml");
    assert!(
        err.to_string()
            .starts_with("pages/heating.yaml: at with.x (param `x`): "),
        "{err}"
    );
}

#[test]
fn query_errors_are_reported() {
    let ctx = Fake::new(DEVICES);
    let doc = yaml("- { for_each: { query: { node: { type: fail } } }, item: 1 }");
    let err = expand(&doc, &TemplateRegistry::new(), &ctx).unwrap_err();
    assert_eq!(err.kind, TemplateErrorKind::Query);
    assert_eq!(err.path.to_string(), "[0].for_each");
}

#[test]
fn check_reports_load_time_errors() {
    let reg = registry(&[
        "{ template: card, params: [room, size], defaults: { size: 1 }, body: { t: '{{ room }}' } }",
        "{ template: self, body: { use: self } }",
        "{ template: a, body: [{ use: b }] }",
        "{ template: b, body: { x: { use: a } } }",
        "{ template: bad-body, body: { t: '{{ 1 + }}' } }",
    ]);
    let doc = yaml(
        r#"
cards:
  - { use: nope }
  - { use: card }
  - { use: card, with: { room: x, colour: red } }
  - { label: "{{ a + }}" }
  - { use: card, with: { room: x }, extra: 1 }
  - { when: "a ==", x: 1 }
  - { for_each: { query: { node: { type: [1, 2] } } }, use: card, with: { room: x } }
  - { for_each: { query: {} } }
  - { use: self }
  - { use: a }
  - { use: bad-body }
  - { for_each: { query: {} }, use: card, with: { room: "{{ item.id }}" } }
"#,
    );
    let errors = check(&doc, &reg);
    let found: Vec<(TemplateErrorKind, String, Option<String>)> = errors
        .iter()
        .map(|e| (e.kind, e.path.to_string(), e.param.clone()))
        .collect();
    let expect = |kind, path: &str, param: Option<&str>| {
        assert!(
            found.contains(&(kind, path.to_owned(), param.map(str::to_owned))),
            "missing {kind:?} at {path}; got {found:#?}"
        );
    };
    expect(TemplateErrorKind::UnknownTemplate, "cards[0]", None);
    expect(TemplateErrorKind::MissingParam, "cards[1]", Some("room"));
    expect(TemplateErrorKind::UnknownParam, "cards[2]", Some("colour"));
    expect(TemplateErrorKind::Syntax, "cards[3].label", None);
    expect(TemplateErrorKind::Malformed, "cards[4]", None);
    expect(TemplateErrorKind::Syntax, "cards[5].when", None);
    expect(
        TemplateErrorKind::Malformed,
        "cards[6].for_each.query",
        None,
    );
    expect(TemplateErrorKind::Malformed, "cards[7]", None);
    expect(TemplateErrorKind::Syntax, "bad-body > t", None);
    let recursion: Vec<&str> = errors
        .iter()
        .filter(|e| e.kind == TemplateErrorKind::Recursion)
        .map(|e| e.message.as_str())
        .collect();
    assert!(
        recursion.contains(&"template recursion: self -> self"),
        "{recursion:?}"
    );
    assert!(
        recursion.contains(&"template recursion: a -> b -> a"),
        "{recursion:?}"
    );
    assert_eq!(recursion.len(), 2, "{recursion:?}");
    // The last for_each is valid; nothing reported at cards[11].
    assert!(
        !found.iter().any(|(_, p, _)| p.starts_with("cards[11]")),
        "{found:#?}"
    );

    let engine = Engine::new();
    let refs = engine.references(&yaml("[{ use: a }, { use: nope }]"), &reg);
    assert_eq!(refs.into_iter().collect::<Vec<_>>(), ["a", "b", "nope"]);

    // Recursion and depth are also caught at expansion.
    let err = expand(&yaml("{ use: a }"), &reg, &EmptyContext).unwrap_err();
    assert_eq!(err.kind, TemplateErrorKind::Recursion);
    assert!(check(&yaml("{ use: card, with: { room: x } }"), &reg).is_empty());
}

#[test]
fn depth_limit_stops_runaway_nesting() {
    let reg = registry(&[
        "{ template: l1, body: { use: l2 } }",
        "{ template: l2, body: { use: l3 } }",
        "{ template: l3, body: done }",
    ]);
    let mut engine = Engine::new();
    assert_eq!(engine.max_depth(), 32);
    assert_eq!(
        engine
            .expand(&yaml("{ use: l1 }"), &reg, &EmptyContext)
            .unwrap(),
        yaml("done")
    );
    engine.set_max_depth(2);
    let err = engine
        .expand(&yaml("{ use: l1 }"), &reg, &EmptyContext)
        .unwrap_err();
    assert_eq!(err.kind, TemplateErrorKind::DepthLimit);
}

#[test]
fn scope_variables_shadow_outer_scopes() {
    let reg = registry(&["{ template: label, body: { text: '{{ device.name }}' } }"]);
    let mut ctx = Fake::new(DEVICES);
    let mut fallback = Mapping::new();
    fallback.insert("name".into(), "global".into());
    ctx.globals
        .insert("device".into(), Value::Mapping(fallback));
    let doc = yaml(
        r#"
title: "{{ device.name }}"
card:
  device: thermo-a
  title: "{{ device.name }}"
  items:
    - { label: "{{ device.name }}" }
    - { device: thermo-b, label: "{{ device.name }}" }
    - { device: "{{ device.id | replace('a', 'c') }}", label: "{{ device.name }}" }
    - { use: label }
"#,
    );
    let expected = yaml(
        r#"
title: global
card:
  device: thermo-a
  title: Office thermostat
  items:
    - { label: Office thermostat }
    - { device: thermo-b, label: Bath thermostat }
    - { device: thermo-c, label: Kitchen thermostat }
    - { text: Office thermostat }
"#,
    );
    let err = expand(&doc, &TemplateRegistry::new(), &ctx).unwrap_err();
    assert_eq!(err.kind, TemplateErrorKind::UnknownTemplate);
    assert_eq!(expand(&doc, &reg, &ctx).unwrap(), expected);
}

#[test]
fn app_registered_functions_and_filters() {
    let mut ctx = Fake::new(DEVICES);
    let mut devices = Mapping::new();
    for d in &ctx.devices {
        devices.insert(d["id"].clone(), d.clone());
    }
    ctx.globals
        .insert("__devices".into(), Value::Mapping(devices));

    let engine = Engine::new().with_env(|env| {
        env.add_function(
            "device",
            |state: &State, id: String| -> Result<MjValue, MjError> {
                let devices = state.lookup("__devices").unwrap_or_default();
                devices.get_attr(&id)
            },
        );
        env.add_filter("shout", |s: String| s.to_uppercase());
    });
    let doc = yaml("{ name: \"{{ device('thermo-a').name | shout }}\", room: \"{{ device('thermo-b').meta.room[0] }}\" }");
    assert_eq!(
        engine.expand(&doc, &TemplateRegistry::new(), &ctx).unwrap(),
        yaml("{ name: OFFICE THERMOSTAT, room: Bath }")
    );
}

#[test]
fn registry_tracks_changes_and_validates_templates() {
    let mut reg = TemplateRegistry::new();
    let t = Template::from_value(&yaml("{ template: t, params: [a], body: 1 }")).unwrap();
    assert!(reg.insert(t.clone()));
    assert!(!reg.insert(t));
    let (name, changed) = reg
        .insert_document(&yaml("{ template: t, params: [a], body: 2 }"))
        .unwrap();
    assert_eq!((name.as_str(), changed), ("t", true));
    assert!(reg.remove("t").is_some());
    assert!(reg.is_empty());

    assert!(Template::is_template_document(&yaml(
        "{ template: x, body: 1 }"
    )));
    assert!(!Template::is_template_document(&yaml("{ cards: [] }")));
    let err = Template::from_value(&yaml(
        "{ template: t, params: [a], defaults: { b: 1 }, body: 1 }",
    ))
    .unwrap_err();
    assert_eq!(err.kind, TemplateErrorKind::InvalidTemplate);
    assert_eq!(err.param.as_deref(), Some("b"));
    assert!(Template::from_value(&yaml("{ template: t, params: [a, a], body: 1 }")).is_err());
    assert!(Template::from_value(&yaml("{ template: t }")).is_err());
    assert!(Template::from_value(&yaml("{ template: t, body: 1, extra: 2 }")).is_err());
}

#[test]
fn heating_room_card_expands_to_fixed_tree() {
    let reg = registry(&[
        r#"
template: temperature-row
params: [device, label]
defaults: { label: Temperature }
body:
  - { type: value, device: "{{ device }}", property: temperature/temperature, label: "{{ label }}" }
"#,
        r#"
template: heating-room-card
params: [item, title, window]
defaults:
  title: "{{ item.meta.room[0] }}"
  window: null
body:
  type: card
  title: "{{ title }}"
  device: "{{ item.id }}"
  columns: 2
  items:
    - { type: thermostat, device: "{{ item.id }}", property: thermostat/set-point, label: "{{ device.name }}" }
    - use: temperature-row
      with: { device: "{{ item.id }}" }
    - when: "window is not none"
      type: contact
      device: "{{ window }}"
      property: contact/state
    - { type: text, text: "{{ title }}: {{ item.meta.room | length }} room tags" }
"#,
    ]);
    let ctx = Fake::new(DEVICES);
    let page = yaml(
        r#"
id: heating
cards:
  - for_each:
      query: { node: { type: thermostat } }
      sort_by: meta.room
    use: heating-room-card
  - use: heating-room-card
    with:
      item: { id: thermo-a, meta: { room: [Office] } }
      title: Office (fixed)
      window: window-office
"#,
    );
    assert!(check(&page, &reg).is_empty(), "{:?}", check(&page, &reg));
    let expected = yaml(
        r#"
id: heating
cards:
  - type: card
    title: Bath
    device: thermo-b
    columns: 2
    items:
      - { type: thermostat, device: thermo-b, property: thermostat/set-point, label: Bath thermostat }
      - { type: value, device: thermo-b, property: temperature/temperature, label: Temperature }
      - { type: text, text: "Bath: 2 room tags" }
  - type: card
    title: B
    device: thermo-c
    columns: 2
    items:
      - { type: thermostat, device: thermo-c, property: thermostat/set-point, label: Kitchen thermostat }
      - { type: value, device: thermo-c, property: temperature/temperature, label: Temperature }
      - { type: text, text: "B: 4 room tags" }
  - type: card
    title: Office
    device: thermo-a
    columns: 2
    items:
      - { type: thermostat, device: thermo-a, property: thermostat/set-point, label: Office thermostat }
      - { type: value, device: thermo-a, property: temperature/temperature, label: Temperature }
      - { type: text, text: "Office: 1 room tags" }
  - type: card
    title: Office (fixed)
    device: thermo-a
    columns: 2
    items:
      - { type: thermostat, device: thermo-a, property: thermostat/set-point, label: Office thermostat }
      - { type: value, device: thermo-a, property: temperature/temperature, label: Temperature }
      - { type: contact, device: window-office, property: contact/state }
      - { type: text, text: "Office (fixed): 1 room tags" }
"#,
    );
    assert_eq!(expand(&page, &reg, &ctx).unwrap(), expected);
}
