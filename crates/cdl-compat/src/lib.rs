//! Compatibility between simplified python-cdl JSON and the canonical CDL model.
//!
//! Instances, declarations and wires carry their own compatibility properties;
//! writing a document reflects model edits and never returns an original graph snapshot.
//! Name resolution uses the pinned CXF-Core interfaces. See the crate README for
//! ACE extensions and the corresponding standard classes.

#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

mod core;
pub mod legacy;
mod read;
mod write;

use cdl_model::{Annotation, Diagnostic, DiagnosticCode, Meta};
pub use read::{from_json, from_value, ReadOptions};
use serde_json::{Map, Value};
pub use write::{to_json, to_value};

const FIELDS: &str = "ace:compat:fields";
const EXTRA: &str = "ace:compat:extra";
const TYPE_KEY: &str = "ace:compat:type_key";
const BLOCK_TYPE: &str = "ace:blockType";
const EQUATIONS: &str = "ace:equations";

/// Load the pinned standard interface definitions for native hosts.
///
/// Compatibility alias for `cxf::core_library()`. The format crate owns the pinned
/// resource; this function adds no behavior or parameter-expression evaluation.
pub fn pinned_core_library() -> cdl_model::Library {
    cxf::core_library()
}

/// Encode the authoritative registry catalog, including implementation availability.
///
/// JSON encoding stays in the format adapter; the block crate depends only on the
/// model. The pinned-core loader itself is owned by `cxf`.
pub fn catalog_json(registry: &cdl_blocks::Registry) -> Result<String, serde_json::Error> {
    serde_json::to_string(&registry.catalog())
}

fn error(path: &str, code: DiagnosticCode, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(path, code, message)
}

fn annotate(value: &Value) -> Annotation {
    // Both sides are serde data models; the model never depends on serde_json.
    serde_json::from_value(value.clone()).expect("JSON values fit annotation data")
}
fn json_annotation(value: &Annotation) -> Value {
    serde_json::to_value(value).expect("JSON-origin annotations are serializable")
}

fn metadata(object: &Map<String, Value>, known: &[&str]) -> Meta {
    let mut meta = Meta {
        label: object
            .get("label")
            .and_then(Value::as_str)
            .map(str::to_owned),
        description: object
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_owned),
        ..Meta::default()
    };
    meta.annotations.insert(
        FIELDS.into(),
        Annotation::Array(
            object
                .keys()
                .map(|key| Annotation::String(key.clone()))
                .collect(),
        ),
    );
    let extra: Map<String, Value> = object
        .iter()
        .filter(|(key, value)| {
            let namespace_property = key.contains(':')
                && !key.starts_with("ace:compat:")
                && key.as_str() != BLOCK_TYPE
                && key.as_str() != EQUATIONS;
            let alias = [
                ("notes", "ace:notes"),
                ("brick_class", "ace:brickClass"),
                ("point_label", "ace:pointLabel"),
            ]
            .iter()
            .any(|(field, property)| key.as_str() == *field && !object.contains_key(*property));
            let text = matches!(key.as_str(), "label" | "description") && value.is_string();
            !namespace_property
                && !alias
                && !text
                && (!known.contains(&key.as_str())
                    || (key.as_str() == "description" && value.is_null()))
        })
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    meta.annotations
        .insert(EXTRA.into(), annotate(&Value::Object(extra)));
    for (key, value) in object {
        if key.contains(':') && !key.starts_with("ace:compat:") {
            meta.annotations.insert(key.clone(), annotate(value));
        }
    }
    for (field, property) in [
        ("notes", "ace:notes"),
        ("brick_class", "ace:brickClass"),
        ("point_label", "ace:pointLabel"),
    ] {
        if !object.contains_key(property) {
            if let Some(value) = object.get(field) {
                meta.annotations.insert(property.into(), annotate(value));
            }
        }
    }
    meta
}
fn original_fields(meta: &Meta) -> Option<Vec<String>> {
    let Annotation::Array(fields) = meta.annotations.get(FIELDS)? else {
        return None;
    };
    Some(
        fields
            .iter()
            .filter_map(|field| match field {
                Annotation::String(s) => Some(s.clone()),
                _ => None,
            })
            .collect(),
    )
}
fn extra(meta: &Meta) -> Map<String, Value> {
    meta.annotations
        .get(EXTRA)
        .map(json_annotation)
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}
fn included(meta: &Meta, field: &str) -> bool {
    original_fields(meta).is_none_or(|fields| fields.iter().any(|name| name == field))
}

fn write_metadata(object: &mut Map<String, Value>, meta: &Meta) {
    if let Some(label) = &meta.label {
        object.insert("label".into(), Value::String(label.clone()));
    }
    if let Some(description) = &meta.description {
        object.insert("description".into(), Value::String(description.clone()));
    }
    let original = original_fields(meta);
    let fields = original.clone().unwrap_or_default();
    for (property, value) in &meta.annotations {
        if property.starts_with("ace:compat:") || property == BLOCK_TYPE || property == EQUATIONS {
            continue;
        }
        let field = [
            ("notes", "ace:notes"),
            ("brick_class", "ace:brickClass"),
            ("point_label", "ace:pointLabel"),
            ("unit", "ace:unit"),
        ]
        .iter()
        .find(|(field, key)| {
            key == property
                && (original.is_none()
                    || (fields.iter().any(|name| name == field)
                        && !fields.iter().any(|name| name == property)))
        })
        .map(|(field, _)| *field)
        .unwrap_or(property);
        object.insert(field.into(), json_annotation(value));
    }
}
