//! Compacted CXF JSON-LD format support, independent of execution and simulation.
//!
//! The bundled core is pinned to modelica-json commit
//! `f75058d25bafb8c503e894dd193d3fa8037aef23`. Its upstream license is included in
//! `resources/modelica-json-LICENSE.md`. This crate does not fetch remote contexts.

#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

pub mod core;
mod format;
mod read;
mod write;
pub use read::{from_json, from_value, from_value_with_root};
pub use write::{to_json, to_value};

/// Read caller-supplied elementary core declarations without the bundled resource.
/// This accepts the compacted modelica-json graph and performs no remote lookup.
pub fn read_core_library(input: &str) -> Result<cdl_model::Library, Vec<cdl_model::Diagnostic>> {
    use cdl_model::{Diagnostic, DiagnosticCode, Enumeration, Library};
    let error = |message: String| {
        vec![Diagnostic::new(
            "@graph",
            DiagnosticCode::InvalidDocument,
            message,
        )]
    };
    let source: serde_json::Value =
        serde_json::from_str(input).map_err(|e| error(e.to_string()))?;
    let normalized = format::Index::parse(&source).map_err(|e| vec![e])?;
    let source = serde_json::json!({"@graph":normalized.order.iter().map(|id|&normalized.nodes[id]).collect::<Vec<_>>()});
    let graph = source["@graph"]
        .as_array()
        .ok_or_else(|| error("expected a compacted CXF @graph array".into()))?;
    let mut index = std::collections::BTreeMap::new();
    for node in graph {
        let id = node["@id"]
            .as_str()
            .ok_or_else(|| error("graph records require string @id".into()))?;
        if index.insert(id, node).is_some() {
            return Err(error(format!("duplicate @id {id}")));
        }
    }
    let mut library = Library::default();
    for node in graph {
        if node["@type"] == "S231:ElementaryBlock" {
            for field in [
                "S231:hasInput",
                "S231:hasOutput",
                "S231:hasParameter",
                "S231:hasConstant",
            ] {
                if let Some(value) = node.get(field) {
                    let records = match value {
                        serde_json::Value::Array(records) => records.iter().collect::<Vec<_>>(),
                        serde_json::Value::Object(_) => vec![value],
                        _ => return Err(error(format!("{field} requires @id references"))),
                    };
                    if records.iter().any(|r| r["@id"].as_str().is_none()) {
                        return Err(error(format!("{field} requires string @id references")));
                    }
                }
                for reference in core::refs(&node[field]) {
                    if !index.contains_key(reference) {
                        return Err(error(format!("missing declaration {reference}")));
                    }
                }
            }
            let class = core::definition(node, &index);
            library.classes.insert(class.id.clone(), class);
        } else if node["@type"] == "S231:EnumerationType" {
            let enumeration = Enumeration {
                id: node["@id"].as_str().unwrap().into(),
                members: vec![],
                meta: core::meta(node),
            };
            library
                .enumerations
                .insert(enumeration.id.clone(), enumeration);
        }
    }
    Ok(library)
}

/// Associated bundled-library construction for the canonical model's `Library`.
///
/// Rust requires an extension trait because the type is owned by `cdl-model`.
pub trait CoreLibrary {
    /// Load the bundled pinned interfaces, without executable implementations.
    #[cfg(feature = "bundled-core")]
    fn core() -> Self;
}
impl CoreLibrary for cdl_model::Library {
    #[cfg(feature = "bundled-core")]
    fn core() -> Self {
        core_library()
    }
}

/// Load the bundled pinned standard interfaces and enumeration identities.
///
/// ```
/// use cdl_model::Library;
/// use cxf::CoreLibrary;
/// let library = Library::core();
/// assert_eq!(library.classes.len(), 132);
/// ```
#[cfg(feature = "bundled-core")]
pub fn core_library() -> cdl_model::Library {
    let mut library = cdl_model::Library::default();
    for interface in core::interfaces() {
        library.classes.insert(
            interface.definition.id.clone(),
            interface.definition.clone(),
        );
        for enumeration in &interface.enumerations {
            library
                .enumerations
                .insert(enumeration.id.clone(), enumeration.clone());
        }
    }
    library
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_core_references_are_diagnostics() {
        let text = r#"{"@graph":[{"@id":"https://example.org/Add","@type":"S231:ElementaryBlock","S231:hasInput":{"@id":"https://example.org/missing"}}]}"#;
        assert!(read_core_library(text).unwrap_err()[0]
            .message
            .contains("missing declaration"));
        assert!(read_core_library("{}").is_err());
        assert!(read_core_library(r#"{"@graph":[{"@id":"https://example.org/Add","@type":"S231:ElementaryBlock","S231:hasInput":[{}]}]}"#).is_err());
    }
    #[test]
    #[cfg(feature = "bundled-core")]
    fn external_and_bundled_core_loaders_agree() {
        let external = read_core_library(include_str!("../resources/CXF-Core.jsonld")).unwrap();
        assert_eq!(external, core_library());
        assert_eq!(external.classes.len(), 132);
    }
}
