//! Structural templates for YAML config trees (feature `template`).
//!
//! The engine works on parsed [`serde_yaml_ng::Value`] trees and knows nothing about Homie:
//! apps load documents, register templates, and expand each document against their own
//! [`ExpandContext`] before deserializing it into typed config.
//!
//! # Constructs
//!
//! | Construct | Where | Effect |
//! |---|---|---|
//! | `"{{ expr }}"` | any scalar string | exactly one placeholder (whitespace around allowed) yields the typed value; mixed text renders a string |
//! | `{ use: name, with: {..} }` | any mapping position | replaced by the template body; in a list a list body is spliced |
//! | `{ for_each: { query, sort_by }, use: name, with: {..} }` | list item, document root | one expansion of the template per element, spliced |
//! | `{ for_each: { query, sort_by }, item: <body> }` | list item, document root | one expansion of `item` per element, spliced |
//! | `when: expr` key | list item, document root | the item is kept (without `when`) only if `expr` is truthy |
//!
//! Keys are never templated. Strings without `{{` are kept verbatim (`{%` and `{#` have no
//! meaning). Expressions use minijinja with strict undefined: an unknown variable or an
//! expression that yields undefined is an error.
//!
//! # Variables and scopes
//!
//! From outermost to innermost (inner shadows outer):
//! 1. [`ExpandContext::globals`];
//! 2. app scopes from [`ExpandContext::scope_vars`] (for example an implicit `device` derived
//!    from a `device:` field), visible in the subtree of their mapping, also inside template
//!    bodies used there;
//! 3. template parameters (`with` values, then `defaults`, which may refer to supplied and
//!    earlier parameters), visible only in that template's body: caller parameters and
//!    `item`s do not leak into a body;
//! 4. `item`, the current `for_each` element. With `use`, `with` values see `item`; a template
//!    that declares a parameter named `item` receives the element automatically unless `with`
//!    sets it.
//!
//! # for_each order
//!
//! Elements come from [`ExpandContext::query`] and are sorted by the `sort_by` field path
//! (dotted, e.g. `meta.room`; a list value sorts by its first element; numbers numerically,
//! strings lexically), then by their `id` field. Missing or null values sort last.
//!
//! # Limits
//!
//! Nested `use` is limited to [`DEFAULT_MAX_DEPTH`] (32) levels ([`Engine::set_max_depth`]);
//! a template using itself is reported as recursion by [`Engine::check`] and at expansion.

// TemplateError carries path, template, param and message by value; it is only built on the
// failure path of load-time checks and renders, so its size does not matter.
#![allow(clippy::result_large_err)]

mod engine;
mod error;
mod expr;
mod registry;

pub use engine::{
    check, expand, expand_documents, EmptyContext, Engine, ExpandContext, DEFAULT_MAX_DEPTH,
};
pub use error::{PathSegment, TemplateError, TemplateErrorKind, YamlPath};
pub use registry::{Template, TemplateRegistry};

/// The minijinja version used by the engine, for registering functions and filters.
pub use minijinja;
