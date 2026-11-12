use std::collections::{BTreeMap, HashSet};

use serde::Deserialize;
use serde_yaml_ng::{Mapping, Value};

use super::error::{PathSegment, TemplateError, TemplateErrorKind};

/// A named, parameterized YAML fragment.
///
/// Document form:
///
/// ```yaml
/// template: heating-room-card
/// params: [room, thermostat]
/// defaults:
///   title: "{{ room }}"      # defaults may use earlier and supplied params
/// body: { ... any YAML ... }
/// ```
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Template {
    #[serde(rename = "template")]
    pub name: String,
    #[serde(default)]
    pub params: Vec<String>,
    /// Default values by parameter name; every key must be a declared parameter.
    #[serde(default)]
    pub defaults: Mapping,
    pub body: Value,
}

impl Template {
    /// Returns `true` when the document is a template document (a mapping with a `template` key).
    pub fn is_template_document(doc: &Value) -> bool {
        doc.as_mapping().is_some_and(|m| m.contains_key("template"))
    }

    /// Parses and validates a template document.
    pub fn from_value(doc: &Value) -> Result<Self, TemplateError> {
        let template: Template = serde_yaml_ng::from_value(doc.clone()).map_err(|e| {
            TemplateError::new(TemplateErrorKind::InvalidTemplate, &[], e.to_string())
        })?;
        template.validate()?;
        Ok(template)
    }

    fn validate(&self) -> Result<(), TemplateError> {
        let path = [PathSegment::Template(self.name.clone())];
        let invalid =
            |msg: String| TemplateError::new(TemplateErrorKind::InvalidTemplate, &path, msg);
        if self.name.is_empty() {
            return Err(invalid("template name is empty".into()));
        }
        let mut seen = HashSet::new();
        for param in &self.params {
            if !seen.insert(param.as_str()) {
                return Err(invalid(format!("parameter `{param}` is declared twice")));
            }
        }
        for key in self.defaults.keys() {
            match key.as_str() {
                Some(k) if seen.contains(k) => {}
                Some(k) => {
                    return Err(
                        invalid(format!("default for undeclared parameter `{k}`")).param(Some(k))
                    )
                }
                None => return Err(invalid("default keys must be strings".into())),
            }
        }
        Ok(())
    }

    /// The default for `param`, if any.
    pub fn default_for(&self, param: &str) -> Option<&Value> {
        self.defaults.get(param)
    }
}

/// Templates by name.
#[derive(Debug, Clone, Default)]
pub struct TemplateRegistry {
    templates: BTreeMap<String, Template>,
}

impl TemplateRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds or replaces a template; returns `true` when the registry changed (new name or a
    /// different definition), so apps re-expand only documents using changed templates.
    pub fn insert(&mut self, template: Template) -> bool {
        match self.templates.get(&template.name) {
            Some(existing) if *existing == template => false,
            _ => {
                self.templates.insert(template.name.clone(), template);
                true
            }
        }
    }

    /// Parses a template document and inserts it; returns the name and whether it changed.
    pub fn insert_document(&mut self, doc: &Value) -> Result<(String, bool), TemplateError> {
        let template = Template::from_value(doc)?;
        let name = template.name.clone();
        let changed = self.insert(template);
        Ok((name, changed))
    }

    /// Removes a template, returning it when it existed.
    pub fn remove(&mut self, name: &str) -> Option<Template> {
        self.templates.remove(name)
    }

    pub fn get(&self, name: &str) -> Option<&Template> {
        self.templates.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.templates.contains_key(name)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.templates.keys().map(String::as_str)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Template> {
        self.templates.values()
    }

    pub fn len(&self) -> usize {
        self.templates.len()
    }

    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }
}
