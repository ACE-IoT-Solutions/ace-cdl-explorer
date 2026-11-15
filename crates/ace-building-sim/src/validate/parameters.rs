//! Temporary bridge for the public legacy `Block` validation API. Parameter
//! semantics belong to cdl-blocks; this module only translates value containers.
use crate::model::Block;
use cdl_blocks::{BlockImpl, ConfiguredInstance, Registry};
use cdl_model::{Annotation, Assignment, BlockClass, BlockKind, Instance, Library, Meta, Value};
use std::sync::OnceLock;

fn value(source: &serde_json::Value) -> Value {
    match source {
        serde_json::Value::Bool(v) => Value::Boolean(*v),
        serde_json::Value::Number(v) => match v.as_i64() {
            Some(v) => Value::Integer(v),
            None => Value::Real(v.as_f64().unwrap_or(f64::NAN)),
        },
        serde_json::Value::String(v) => Value::String(v.clone()),
        serde_json::Value::Array(v) => Value::Array(v.iter().map(value).collect()),
        // Unsupported legacy literals must remain unsupported in the registry.
        other => Value::Expr(other.to_string()),
    }
}

pub(super) fn check(block: &Block, path: &str, errors: &mut Vec<String>) {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    let registry = REGISTRY.get_or_init(|| Registry::new(Library::default()));
    let family = block.block_type.rsplit('.').next().unwrap();
    let Some(definition) = registry.legacy(family) else {
        return;
    };
    let mut class = BlockClass::new(definition.class().clone(), BlockKind::Extension);
    class.meta.annotations.insert(
        "ace:blockType".into(),
        Annotation::String(block.block_type.clone()),
    );
    let mut instance = Instance::new(&block.name, class.id.clone());
    instance.assignments = block
        .parameters
        .iter()
        .map(|p| Assignment {
            name: p.name.clone(),
            value: value(&p.value),
            meta: Meta::default(),
        })
        .collect();
    let config = ConfiguredInstance::new(&instance, &class, path);
    let mut diagnostics = Vec::new();
    definition.check(&config, &mut diagnostics);
    errors.extend(diagnostics.into_iter().map(|e| e.to_string()));
}
