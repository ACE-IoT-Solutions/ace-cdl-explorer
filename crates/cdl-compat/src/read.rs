use crate::*;
use cdl_model::{
    Assignment, BlockClass, BlockKind, ClassId, CompositeBody, Connection, Connector, DataType,
    Dimension, Direction, Document, Endpoint, Instance, Library, ParameterDecl, Units,
    Value as ModelValue,
};
use std::collections::HashMap;

/// Options controlling structural sharing; default preserves independent editor instances.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReadOptions {
    /// Deduplicate equivalent composite definitions, preserving per-instance metadata.
    pub deduplicate_composites: bool,
}

/// Parse simplified JSON into a canonical document.
pub fn from_json(json: &str, options: ReadOptions) -> Result<Document, Vec<Diagnostic>> {
    let value: Value = serde_json::from_str(json)
        .map_err(|e| vec![error("", DiagnosticCode::InvalidDocument, e.to_string())])?;
    from_value(&value, options)
}

/// Convert a parsed simplified graph, preserving unknown fields at their own locations.
pub fn from_value(value: &Value, options: ReadOptions) -> Result<Document, Vec<Diagnostic>> {
    let mut reader = Reader {
        library: Library::default(),
        errors: Vec::new(),
        options,
        signatures: HashMap::new(),
    };
    let root = reader.instance(value, "");
    if reader.errors.is_empty() {
        let mut document = Document {
            library: reader.library,
            root,
        };
        materialize_defaults(&mut document);
        Ok(document)
    } else {
        Err(reader.errors)
    }
}

fn materialize_defaults(document: &mut Document) {
    let registry = cdl_blocks::Registry::new(document.library.clone());
    let fill = |instance: &mut Instance| {
        let Some(class) = registry.library().classes.get(&instance.class) else {
            return;
        };
        if class.body.is_some() {
            return;
        }
        let config = cdl_blocks::ConfiguredInstance::new(instance, class, &instance.name);
        if let Ok(defaults) = registry.legacy_defaults(&config) {
            instance.assignments.extend(defaults);
        }
    };
    fill(&mut document.root);
    for class in document.library.classes.values_mut() {
        if let Some(body) = &mut class.body {
            for instance in &mut body.instances {
                fill(instance);
            }
        }
    }
}

struct Reader {
    library: Library,
    errors: Vec<Diagnostic>,
    options: ReadOptions,
    signatures: HashMap<String, ClassId>,
}

fn array<'a>(object: &'a Map<String, Value>, field: &str) -> &'a [Value] {
    object
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}
fn text(object: &Map<String, Value>, field: &str) -> String {
    object
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}
fn data_type(kind: &str, value: Option<&Value>) -> DataType {
    match kind {
        "Boolean" => DataType::Boolean,
        "Integer" => DataType::Integer,
        "String" => DataType::String,
        "Real" => DataType::Real,
        _ => match value {
            Some(Value::Bool(_)) => DataType::Boolean,
            Some(Value::Number(number)) if number.is_i64() || number.is_u64() => DataType::Integer,
            Some(Value::String(_)) => DataType::String,
            Some(Value::Array(values)) => data_type("", values.first()),
            _ => DataType::Real,
        },
    }
}
fn model_value(value: &Value, kind: &DataType) -> ModelValue {
    match value {
        Value::Bool(v) => ModelValue::Boolean(*v),
        Value::Number(v)
            if *kind == DataType::Integer
                && v.is_f64()
                && v.as_f64().is_some_and(|n| {
                    n.is_finite() && n.fract() == 0.0 && n >= i64::MIN as f64 && n < i64::MAX as f64
                }) =>
        {
            ModelValue::Integer(v.as_f64().unwrap() as i64)
        }
        Value::Number(v) => v
            .as_i64()
            .map(ModelValue::Integer)
            .unwrap_or_else(|| ModelValue::Real(v.as_f64().unwrap())),
        Value::String(v) if *kind == DataType::String => ModelValue::String(v.clone()),
        Value::String(v) => ModelValue::Expr(v.clone()),
        Value::Array(values) => {
            ModelValue::Array(values.iter().map(|v| model_value(v, kind)).collect())
        }
        _ => ModelValue::Expr(value.to_string()),
    }
}
fn retain_literal(meta: &mut Meta, field: &str, raw: &Value, value: &ModelValue) {
    if crate::write::model_value(value) != *raw {
        meta.annotations.insert(
            format!("ace:compat:literal:{field}:original"),
            annotate(raw),
        );
        meta.annotations.insert(
            format!("ace:compat:literal:{field}:canonical"),
            annotate(&serde_json::to_value(value).unwrap()),
        );
    } else {
        meta.annotations
            .shift_remove(&format!("ace:compat:literal:{field}:original"));
        meta.annotations
            .shift_remove(&format!("ace:compat:literal:{field}:canonical"));
    }
}
fn expand_unit(id: &str) -> String {
    if let Some(name) = id.strip_prefix("unit:") {
        format!("http://qudt.org/vocab/unit#{name}")
    } else if let Some(name) = id.strip_prefix("q:") {
        format!("http://qudt.org/vocab/quantitykind#{name}")
    } else {
        id.into()
    }
}
fn retain_unit_spelling(meta: &mut Meta, object: &Map<String, Value>) {
    for field in ["cxf_unit", "cxf_display_unit", "quantity_kind"] {
        if let Some(raw) = object.get(field) {
            meta.annotations
                .insert(format!("ace:compat:unit:{field}"), annotate(raw));
        }
    }
}
fn units(object: &Map<String, Value>) -> Units {
    let symbol = object.get("unit").and_then(Value::as_str);
    let unit = object
        .get("cxf_unit")
        .and_then(Value::as_str)
        .map(expand_unit)
        .or_else(|| {
            symbol
                .and_then(|symbol| match symbol {
                    "°C" => Some("DEG_C"),
                    "K" => Some("K"),
                    "s" => Some("SEC"),
                    "W" => Some("W"),
                    "kW" => Some("KiloW"),
                    "Pa" => Some("PA"),
                    "%" => Some("PERCENT"),
                    _ => None,
                })
                .map(|name| format!("http://qudt.org/vocab/unit#{name}"))
        });
    Units {
        unit,
        display_unit: object
            .get("cxf_display_unit")
            .and_then(Value::as_str)
            .map(expand_unit),
        quantity_kind: object
            .get("quantity_kind")
            .and_then(Value::as_str)
            .map(expand_unit),
    }
}
fn connector(value: &Value, direction: Direction) -> Connector {
    let object = value.as_object().cloned().unwrap_or_default();
    let mut port = Connector::new(
        text(&object, "name"),
        direction,
        data_type(&text(&object, "type"), None),
    );
    port.units = units(&object);
    port.dimensions = dimensions(&object);
    // Legacy validation requires child inputs but permits unused boundary outputs.
    port.required = direction == Direction::Input;
    port.meta = metadata(
        &object,
        &[
            "name",
            "type",
            "cxf_unit",
            "cxf_display_unit",
            "quantity_kind",
        ],
    );
    retain_unit_spelling(&mut port.meta, &object);
    port.meta.annotations.insert(
        "ace:compat:units".into(),
        annotate(&serde_json::to_value(&port.units).unwrap()),
    );
    port
}
fn dimensions(object: &Map<String, Value>) -> Vec<Dimension> {
    array(object, "dimensions")
        .iter()
        .map(|v| {
            v.as_u64().map(Dimension::Size).unwrap_or_else(|| {
                Dimension::Expr(
                    v.as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| v.to_string()),
                )
            })
        })
        .collect()
}
fn parameter(value: &Value) -> (ParameterDecl, Assignment) {
    let object = value.as_object().cloned().unwrap_or_default();
    let raw = object.get("value").unwrap_or(&Value::Null);
    let kind = data_type(&text(&object, "type"), Some(raw));
    let value = model_value(raw, &kind);
    let dimensions = if object.contains_key("dimensions") {
        dimensions(&object)
    } else {
        fn shape(value: &Value) -> Vec<Dimension> {
            if let Value::Array(values) = value {
                let mut dimensions = vec![Dimension::Size(values.len() as u64)];
                if let Some(first) = values.first() {
                    dimensions.extend(shape(first));
                }
                dimensions
            } else {
                Vec::new()
            }
        }
        shape(raw)
    };
    let mut meta = metadata(
        &object,
        &[
            "name",
            "value",
            "type",
            "cxf_unit",
            "cxf_display_unit",
            "quantity_kind",
            "min",
            "max",
            "dimensions",
        ],
    );
    meta.annotations.insert(
        "ace:compat:units".into(),
        annotate(&serde_json::to_value(units(&object)).unwrap()),
    );
    if let Some(kind) = object.get("type") {
        meta.annotations
            .insert("ace:compat:type".into(), annotate(kind));
    }
    meta.annotations.insert(
        "ace:compat:canonical_type".into(),
        annotate(&serde_json::to_value(&kind).unwrap()),
    );
    meta.annotations.insert(
        "ace:compat:dimensions".into(),
        annotate(&serde_json::to_value(&dimensions).unwrap()),
    );
    retain_unit_spelling(&mut meta, &object);
    retain_literal(&mut meta, "value", raw, &value);
    for field in ["min", "max"] {
        if let Some(raw) = object.get(field) {
            retain_literal(&mut meta, field, raw, &model_value(raw, &kind));
        }
    }
    let declaration = ParameterDecl {
        name: text(&object, "name"),
        data_type: kind.clone(),
        default: None,
        units: units(&object),
        min: object.get("min").map(|v| model_value(v, &kind)),
        max: object.get("max").map(|v| model_value(v, &kind)),
        dimensions,
        meta: meta.clone(),
    };
    let assignment = Assignment {
        name: declaration.name.clone(),
        value,
        meta: Meta {
            annotations: meta
                .annotations
                .into_iter()
                .filter(|(key, _)| key.starts_with("ace:compat:literal:value:"))
                .collect(),
            ..Meta::default()
        },
    };
    (declaration, assignment)
}
fn port_signature(ports: &[Connector]) -> Vec<(String, String)> {
    let mut result: Vec<_> = ports
        .iter()
        .map(|p| {
            (
                p.name.clone(),
                match p.data_type {
                    DataType::Real => "Real",
                    DataType::Integer => "Integer",
                    DataType::Boolean => "Boolean",
                    _ => "String",
                }
                .to_owned(),
            )
        })
        .collect();
    result.sort();
    result
}
fn canonical_signature(ports: &[(String, String)]) -> Vec<(String, String)> {
    let mut ports = ports.to_vec();
    ports.sort();
    ports
}

impl Reader {
    fn resolve(&mut self, kind: &str, class: &BlockClass, path: &str, equations: bool) -> ClassId {
        let full = kind
            .strip_prefix("S231:")
            .map(|name| format!("{}{name}", core::STANDARD))
            .unwrap_or_else(|| kind.replace("http://example.org#", core::STANDARD));
        let qualified = kind.contains('.') || kind.contains(':') || kind.contains('#');
        let short = kind.rsplit(['.', '#']).next().unwrap_or(kind);
        let candidates: Vec<_> = core::interfaces()
            .iter()
            .filter(|entry| {
                if qualified {
                    entry.id == full || entry.id == format!("{}{kind}", core::STANDARD)
                } else {
                    entry.id.rsplit('.').next() == Some(short)
                }
            })
            .collect();
        let inputs = port_signature(&class.inputs);
        let outputs = port_signature(&class.outputs);
        let typed: Vec<_> = candidates
            .iter()
            .copied()
            .filter(|entry| {
                // Types disambiguate names before ACE interface deviations are classified.
                let compatible = |supplied: &[(String, String)], declared: &[(String, String)]| {
                    supplied.iter().all(|(name, kind)| {
                        declared
                            .iter()
                            .find(|(n, _)| n == name)
                            .is_none_or(|(_, k)| k == kind)
                    })
                };
                compatible(&inputs, &entry.inputs) && compatible(&outputs, &entry.outputs)
            })
            .collect();
        if typed.len() > 1 && !equations {
            self.errors.push(error(
                path,
                DiagnosticCode::AmbiguousClass,
                format!(
                    "ambiguous class {kind}: {}",
                    typed
                        .iter()
                        .map(|e| e.id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
        }
        if !equations && typed.len() == 1 {
            let entry = typed[0];
            let exact = inputs == canonical_signature(&entry.inputs)
                && outputs == canonical_signature(&entry.outputs)
                && class.constants.is_empty()
                && class
                    .parameters
                    .iter()
                    .all(|p| entry.parameters.contains(&p.name));
            if exact {
                return entry.id.clone().into();
            }
        }
        // A signature identifies an ACE interface variant, not one particular instance.
        let signature = serde_json::to_string(&(
            kind,
            &class.inputs,
            &class.outputs,
            class
                .parameters
                .iter()
                .map(|p| (&p.name, &p.data_type, &p.dimensions))
                .collect::<Vec<_>>(),
            equations,
        ))
        .unwrap();
        // Stable FNV-1a is used only for local identifiers; collisions are checked below.
        let hash = signature.bytes().fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
        });
        format!(
            "{}{}.{hash:016x}",
            core::ACE,
            if equations {
                "Equations".to_owned()
            } else {
                encode_path(short)
            }
        )
        .into()
    }

    fn instance(&mut self, value: &Value, parent: &str) -> Instance {
        let Some(object) = value.as_object() else {
            self.errors.push(error(
                parent,
                DiagnosticCode::InvalidDocument,
                "block must be an object",
            ));
            return Instance::new("", "");
        };
        for field in [
            "blocks",
            "inputs",
            "outputs",
            "parameters",
            "constants",
            "connections",
            "equations",
        ] {
            if object.get(field).is_some_and(|v| !v.is_array()) {
                self.errors.push(error(
                    parent,
                    DiagnosticCode::InvalidDocument,
                    format!("{field} must be an array"),
                ));
            }
        }
        for field in [
            "inputs",
            "outputs",
            "parameters",
            "constants",
            "connections",
            "equations",
        ] {
            for item in array(object, field) {
                let Some(record) = item.as_object() else {
                    self.errors.push(error(
                        parent,
                        DiagnosticCode::InvalidDocument,
                        format!("{field} entries must be objects"),
                    ));
                    continue;
                };
                let required = match field {
                    "inputs" | "outputs" => vec!["name", "type"],
                    "parameters" | "constants" => vec!["name"],
                    "equations" => vec!["lhs", "rhs"],
                    _ => vec![],
                };
                for key in required {
                    if !record.get(key).is_some_and(Value::is_string) {
                        self.errors.push(error(
                            parent,
                            DiagnosticCode::InvalidDocument,
                            format!("{field}.{key} must be a string"),
                        ));
                    }
                }
                if matches!(field, "parameters" | "constants") && !record.contains_key("value") {
                    self.errors.push(error(
                        parent,
                        DiagnosticCode::InvalidDocument,
                        format!("{field} entry has no value"),
                    ));
                }
                if field == "connections" {
                    for key in [
                        "from",
                        "to",
                        "from_block",
                        "from_output",
                        "to_block",
                        "to_input",
                    ] {
                        if record.get(key).is_some_and(|value| !value.is_string()) {
                            self.errors.push(error(
                                parent,
                                DiagnosticCode::InvalidDocument,
                                format!("{key} must be a string"),
                            ));
                        }
                    }
                }
            }
        }
        let name = text(object, "name");
        let path = if parent.is_empty() {
            name.clone()
        } else {
            format!("{parent}.{name}")
        };
        let type_key = if object.contains_key("block_type") {
            "block_type"
        } else {
            "type"
        };
        let kind = text(object, type_key);
        if name.is_empty() {
            self.errors.push(error(
                &path,
                DiagnosticCode::InvalidName,
                "empty block name",
            ));
        }
        if kind.is_empty() {
            self.errors.push(error(
                &path,
                DiagnosticCode::InvalidDocument,
                "missing block type",
            ));
        }
        let composite = !array(object, "blocks").is_empty()
            || object.get("category").is_some_and(|v| v == "composite");
        let mut class = BlockClass::new(
            "",
            if composite {
                BlockKind::Composite
            } else {
                BlockKind::Elementary
            },
        );
        class.inputs = array(object, "inputs")
            .iter()
            .map(|v| connector(v, Direction::Input))
            .collect();
        class.outputs = array(object, "outputs")
            .iter()
            .map(|v| connector(v, Direction::Output))
            .collect();
        let parameters: Vec<_> = array(object, "parameters").iter().map(parameter).collect();
        class.parameters = parameters.iter().map(|p| p.0.clone()).collect();
        class.constants = array(object, "constants")
            .iter()
            .map(|value| {
                let (mut declaration, assignment) = parameter(value);
                declaration.default = Some(assignment.value);
                declaration
            })
            .collect();
        let mut instance = Instance::new(&name, "");
        instance.assignments = parameters.into_iter().map(|p| p.1).collect();
        instance.meta = metadata(
            object,
            &[
                "name",
                "block_type",
                "type",
                "inputs",
                "outputs",
                "parameters",
                "constants",
                "blocks",
                "connections",
                "equations",
                "description",
            ],
        );
        instance
            .meta
            .annotations
            .insert(TYPE_KEY.into(), Annotation::String(type_key.into()));
        instance
            .meta
            .annotations
            .insert(BLOCK_TYPE.into(), Annotation::String(kind.clone()));
        if object.contains_key("equations") {
            instance
                .meta
                .annotations
                .insert(EQUATIONS.into(), annotate(&object["equations"]));
        }
        let equations = !array(object, "equations").is_empty();
        if composite {
            class.id = format!("{}Composite.{}", core::ACE, encode_path(&path)).into();
            let children = array(object, "blocks")
                .iter()
                .map(|v| self.instance(v, &path))
                .collect();
            let connections =
                array(object, "connections")
                    .iter()
                    .map(|wire| {
                        let object = wire.as_object().cloned().unwrap_or_default();
                        let endpoint =
                            |side: &str| {
                                let path_key = if side == "from" { "from" } else { "to" };
                                let block_key = if side == "from" {
                                    "from_block"
                                } else {
                                    "to_block"
                                };
                                let port_key = if side == "from" {
                                    "from_output"
                                } else {
                                    "to_input"
                                };
                                if object
                                    .get(path_key)
                                    .and_then(Value::as_str)
                                    .is_some_and(|s| !s.is_empty())
                                {
                                    let end = Endpoint::parse(&text(&object, path_key));
                                    if end.instance.as_ref().is_some_and(|name| {
                                        class
                                            .inputs
                                            .iter()
                                            .chain(&class.outputs)
                                            .any(|p| &p.name == name && p.name == end.connector)
                                    }) {
                                        Endpoint::boundary(end.instance.unwrap())
                                    } else {
                                        end
                                    }
                                } else {
                                    let owner = text(&object, block_key);
                                    let port = text(&object, port_key);
                                    if class.inputs.iter().chain(&class.outputs).any(|p| {
                                        p.name == owner && (port.is_empty() || p.name == port)
                                    }) {
                                        Endpoint::boundary(owner)
                                    } else {
                                        Endpoint::child(owner, port)
                                    }
                                }
                            };
                        let a = endpoint("from");
                        let b = endpoint("to");
                        let mut meta = metadata(&object, &[]);
                        meta.annotations.insert(
                            "ace:compat:original_a".into(),
                            annotate(&serde_json::to_value(&a).unwrap()),
                        );
                        meta.annotations.insert(
                            "ace:compat:original_b".into(),
                            annotate(&serde_json::to_value(&b).unwrap()),
                        );
                        Connection { a, b, meta }
                    })
                    .collect();
            class.body = Some(CompositeBody {
                instances: children,
                connections,
            });
        } else {
            class.id = self.resolve(&kind, &class, &path, equations);
            if let Some(interface) = core::interfaces()
                .iter()
                .find(|interface| interface.id == class.id.0)
            {
                instance.connectors = class.inputs.iter().chain(&class.outputs).cloned().collect();
                instance.parameters = class.parameters.clone();
                for declaration in &mut instance.parameters {
                    if let Some(standard) = interface
                        .definition
                        .parameters
                        .iter()
                        .find(|p| p.name == declaration.name)
                    {
                        declaration.data_type = standard.data_type.clone();
                        let baseline =
                            annotate(&serde_json::to_value(&declaration.data_type).unwrap());
                        declaration
                            .meta
                            .annotations
                            .insert("ace:compat:canonical_type".into(), baseline.clone());
                        if let Some(binding) = instance
                            .assignments
                            .iter_mut()
                            .find(|p| p.name == declaration.name)
                        {
                            binding
                                .meta
                                .annotations
                                .insert("ace:compat:canonical_type".into(), baseline);
                        }

                        if let Some(binding) = instance
                            .assignments
                            .iter_mut()
                            .find(|p| p.name == declaration.name)
                        {
                            if let Some(raw) = array(object, "parameters")
                                .iter()
                                .find(|p| p["name"] == declaration.name)
                                .and_then(|p| p.get("value"))
                            {
                                binding.value = model_value(raw, &declaration.data_type);
                                retain_literal(&mut binding.meta, "value", raw, &binding.value);
                            }
                        }
                    }
                }
                for enumeration in &interface.enumerations {
                    self.library
                        .enumerations
                        .insert(enumeration.id.clone(), enumeration.clone());
                }
                class = interface.definition.clone();
            }

            if class.id.0.starts_with(core::ACE) {
                class.kind = BlockKind::Extension;
            }
            class.breaks_algebraic_loop =
                matches!(kind.rsplit('.').next(), Some("UnitDelay" | "Pre"));
        }
        if class.kind == BlockKind::Extension {
            class
                .meta
                .annotations
                .insert(BLOCK_TYPE.into(), Annotation::String(kind.clone()));
        }
        if equations {
            class
                .meta
                .annotations
                .insert(EQUATIONS.into(), annotate(&object["equations"]));
        }
        if self.options.deduplicate_composites && composite {
            let mut definition = class.clone();
            definition.id = ClassId::default();
            let signature = serde_json::to_string(&definition).unwrap();
            if let Some(id) = self.signatures.get(&signature) {
                class.id = id.clone();
            } else {
                self.signatures.insert(signature, class.id.clone());
            }
        }
        instance.class = class.id.clone();
        // Different parameter values are assignments. Interfaces with differing declaration
        // metadata require instance-specific classes until their overrides are represented.
        if let Some(existing) = self.library.classes.get(&class.id) {
            if existing != &class {
                class.id = format!("{}Variant.{}", core::ACE, encode_path(&path)).into();
                class.kind = if composite {
                    BlockKind::Composite
                } else {
                    BlockKind::Extension
                };
                instance.class = class.id.clone();
            }
        }
        self.library.classes.insert(class.id.clone(), class);
        instance
    }
}
fn encode_path(path: &str) -> String {
    path.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
