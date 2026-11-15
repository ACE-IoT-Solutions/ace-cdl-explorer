use crate::*;
use cdl_model::{
    Assignment, BlockClass, Connector, DataType, Document, Endpoint, Instance, ParameterDecl,
    Units, Value as ModelValue,
};
use std::collections::HashSet;

/// Serialize a canonical document to simplified JSON, honoring per-field compatibility metadata.
pub fn to_json(document: &Document) -> Result<String, Vec<Diagnostic>> {
    serde_json::to_string(&to_value(document)?)
        .map_err(|e| vec![error("", DiagnosticCode::InvalidDocument, e.to_string())])
}

/// Export a simplified graph reflecting the current model, including model edits.
pub fn to_value(document: &Document) -> Result<Value, Vec<Diagnostic>> {
    let mut writer = Writer {
        document,
        active: HashSet::new(),
    };
    writer
        .instance(&document.root, &document.root.name)
        .map_err(|e| vec![e])
}
fn kind(kind: &DataType) -> String {
    match kind {
        DataType::Real => "Real",
        DataType::Integer => "Integer",
        DataType::Boolean => "Boolean",
        DataType::String => "String",
        DataType::Enumeration(id) => return id.0.clone(),
    }
    .into()
}
fn parameter_kind(data_type: &DataType, meta: &Meta) -> String {
    let original = meta
        .annotations
        .get("ace:compat:canonical_type")
        .and_then(|value| serde_json::from_value::<DataType>(json_annotation(value)).ok());
    if original.as_ref() == Some(data_type) {
        if let Some(Annotation::String(original)) = meta.annotations.get("ace:compat:type") {
            return original.clone();
        }
    }
    kind(data_type)
}
pub(crate) fn model_value(value: &ModelValue) -> Value {
    match value {
        ModelValue::Real(v) => Value::from(*v),
        ModelValue::Integer(v) => Value::from(*v),
        ModelValue::Boolean(v) => Value::Bool(*v),
        ModelValue::String(v) | ModelValue::Expr(v) | ModelValue::Enum(v) => {
            Value::String(v.clone())
        }
        ModelValue::Array(values) => Value::Array(values.iter().map(model_value).collect()),
    }
}
fn literal_value(meta: &Meta, field: &str, value: &ModelValue) -> Value {
    meta.annotations
        .get(&format!("ace:compat:literal:{field}:canonical"))
        .and_then(|baseline| serde_json::from_value::<ModelValue>(json_annotation(baseline)).ok())
        .filter(|baseline| baseline == value)
        .and_then(|_| {
            meta.annotations
                .get(&format!("ace:compat:literal:{field}:original"))
        })
        .map(json_annotation)
        .unwrap_or_else(|| model_value(value))
}
fn put_units(object: &mut Map<String, Value>, units: &Units, meta: &Meta) {
    let previous = meta
        .annotations
        .get("ace:compat:units")
        .and_then(|value| serde_json::from_value::<Units>(json_annotation(value)).ok());
    for (key, value, baseline) in [
        (
            "cxf_unit",
            &units.unit,
            previous.as_ref().and_then(|p| p.unit.as_ref()),
        ),
        (
            "cxf_display_unit",
            &units.display_unit,
            previous.as_ref().and_then(|p| p.display_unit.as_ref()),
        ),
        (
            "quantity_kind",
            &units.quantity_kind,
            previous.as_ref().and_then(|p| p.quantity_kind.as_ref()),
        ),
    ] {
        let changed = previous.is_some() && baseline != value.as_ref();
        if included(meta, key) || changed {
            let original = (!changed)
                .then(|| meta.annotations.get(&format!("ace:compat:unit:{key}")))
                .flatten();
            if let Some(original) = original {
                object.insert(key.into(), json_annotation(original));
            } else if let Some(value) = value {
                object.insert(key.into(), Value::String(value.clone()));
            } else {
                object.remove(key);
            }
        }
    }
    let changed = previous
        .as_ref()
        .is_some_and(|p| p.unit != units.unit || p.display_unit != units.display_unit);
    if changed {
        object.remove("unit");
    }
    if !object.contains_key("unit") && (changed || original_fields(meta).is_none()) {
        if let Some(unit) = units.display_unit.as_ref().or(units.unit.as_ref()) {
            let local = unit
                .strip_prefix("http://qudt.org/vocab/unit#")
                .or_else(|| unit.strip_prefix("http://qudt.org/vocab/unit/"))
                .or_else(|| unit.strip_prefix("unit:"))
                .or_else(|| unit.rsplit_once('#').map(|(_, local)| local))
                .unwrap_or(unit);
            let symbol = match local {
                "DEG_C" => "°C",
                "K" => "K",
                "SEC" => "s",
                "W" => "W",
                "KiloW" => "kW",
                "PA" => "Pa",
                "PERCENT" => "%",
                _ => local,
            };
            object.insert("unit".into(), Value::String(symbol.into()));
        }
    }
}

fn connector(port: &Connector) -> Value {
    let mut object = extra(&port.meta);
    write_metadata(&mut object, &port.meta);
    if original_fields(&port.meta).is_none() {
        if let Some(Annotation::String(label)) = port.meta.annotations.get("ace:pointLabel") {
            object.insert("label".into(), Value::String(label.clone()));
        }
    }
    object.insert("name".into(), Value::String(port.name.clone()));
    object.insert("type".into(), Value::String(kind(&port.data_type)));
    if !port.dimensions.is_empty()
        || included(&port.meta, "dimensions") && original_fields(&port.meta).is_some()
    {
        object.insert(
            "dimensions".into(),
            Value::Array(
                port.dimensions
                    .iter()
                    .map(|dimension| match dimension {
                        cdl_model::Dimension::Size(size) => Value::from(*size),
                        cdl_model::Dimension::Expr(expression) => Value::String(expression.clone()),
                    })
                    .collect(),
            ),
        );
    }
    put_units(&mut object, &port.units, &port.meta);
    Value::Object(object)
}
fn parameter(declaration: &ParameterDecl, assignment: Option<&Assignment>) -> Value {
    let meta = &declaration.meta;
    let mut object = extra(meta);
    write_metadata(&mut object, meta);
    if let Some(assignment) = assignment {
        object.extend(extra(&assignment.meta));
        write_metadata(&mut object, &assignment.meta);
    }
    object.insert("name".into(), Value::String(declaration.name.clone()));
    if let Some(value) = assignment
        .map(|a| &a.value)
        .or(declaration.default.as_ref())
    {
        object.insert(
            "value".into(),
            literal_value(
                assignment.map_or(meta, |assignment| &assignment.meta),
                "value",
                value,
            ),
        );
    }
    if included(meta, "type") {
        object.insert(
            "type".into(),
            Value::String(parameter_kind(&declaration.data_type, meta)),
        );
    }
    for (key, value) in [("min", &declaration.min), ("max", &declaration.max)] {
        if let Some(value) = value {
            object.insert(key.into(), literal_value(meta, key, value));
        }
    }
    let original_dimensions = meta
        .annotations
        .get("ace:compat:dimensions")
        .and_then(|value| {
            serde_json::from_value::<Vec<cdl_model::Dimension>>(json_annotation(value)).ok()
        });
    if (included(meta, "dimensions") && original_fields(meta).is_some())
        || original_dimensions
            .as_ref()
            .is_some_and(|original| original != &declaration.dimensions)
        || (original_fields(meta).is_none() && !declaration.dimensions.is_empty())
    {
        object.insert(
            "dimensions".into(),
            Value::Array(
                declaration
                    .dimensions
                    .iter()
                    .map(|d| match d {
                        cdl_model::Dimension::Size(n) => Value::from(*n),
                        cdl_model::Dimension::Expr(s) => Value::String(s.clone()),
                    })
                    .collect(),
            ),
        );
    }
    put_units(&mut object, &declaration.units, meta);
    Value::Object(object)
}
struct Writer<'a> {
    document: &'a Document,
    active: HashSet<cdl_model::ClassId>,
}
impl Writer<'_> {
    fn instance(&mut self, instance: &Instance, path: &str) -> Result<Value, Diagnostic> {
        let Some(class) = self.document.library.classes.get(&instance.class) else {
            return Err(error(
                path,
                DiagnosticCode::UnknownClass,
                format!("unknown class {}", instance.class),
            ));
        };
        if !self.active.insert(instance.class.clone()) {
            return Err(error(
                path,
                DiagnosticCode::RecursiveClass,
                "recursive class instantiation",
            ));
        }
        let result = self.block(instance, class, path);
        self.active.remove(&instance.class);
        result
    }
    fn block(
        &mut self,
        instance: &Instance,
        class: &BlockClass,
        path: &str,
    ) -> Result<Value, Diagnostic> {
        let meta = &instance.meta;
        let mut object = extra(meta);
        write_metadata(&mut object, meta);
        object.insert("name".into(), Value::String(instance.name.clone()));
        let type_key = match meta.annotations.get(TYPE_KEY) {
            Some(Annotation::String(key)) => key.as_str(),
            _ => "block_type",
        };
        let block_type = match meta
            .annotations
            .get(BLOCK_TYPE)
            .or_else(|| class.meta.annotations.get(BLOCK_TYPE))
        {
            Some(Annotation::String(kind)) => kind.clone(),
            _ => class
                .id
                .0
                .strip_prefix(core::STANDARD)
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    class
                        .meta
                        .label
                        .clone()
                        .unwrap_or_else(|| class.id.0.clone())
                }),
        };
        object.insert(type_key.into(), Value::String(block_type));
        if let Some(description) = &meta.description {
            object.insert("description".into(), Value::String(description.clone()));
        }
        for (field, direction, declared) in [
            ("inputs", cdl_model::Direction::Input, &class.inputs),
            ("outputs", cdl_model::Direction::Output, &class.outputs),
        ] {
            let ports: Vec<_> = if original_fields(meta).is_none() {
                declared
                    .iter()
                    .map(|p| {
                        instance
                            .connectors
                            .iter()
                            .find(|o| o.name == p.name)
                            .unwrap_or(p)
                    })
                    .chain(instance.connectors.iter().filter(|p| {
                        p.direction == direction && !declared.iter().any(|d| d.name == p.name)
                    }))
                    .collect()
            } else if instance.connectors.is_empty() {
                declared.iter().collect()
            } else {
                instance
                    .connectors
                    .iter()
                    .filter(|port| port.direction == direction)
                    .collect()
            };
            if included(meta, field) || !ports.is_empty() {
                object.insert(
                    field.into(),
                    Value::Array(ports.into_iter().map(connector).collect()),
                );
            }
        }
        let visible_assignments: Vec<_> = instance
            .assignments
            .iter()
            .filter(|assignment| !cdl_blocks::implicit_assignment_unchanged(assignment))
            .collect();
        if included(meta, "parameters") || !visible_assignments.is_empty() {
            let mut parameters = Vec::new();
            for assignment in visible_assignments {
                let Some(declaration) = instance
                    .parameters
                    .iter()
                    .find(|p| p.name == assignment.name)
                    .or_else(|| class.parameters.iter().find(|p| p.name == assignment.name))
                else {
                    return Err(error(
                        path,
                        DiagnosticCode::UnknownParameter,
                        format!("unknown parameter {}", assignment.name),
                    ));
                };
                parameters.push(parameter(declaration, Some(assignment)));
            }
            if original_fields(meta).is_none() {
                parameters.extend(
                    class
                        .parameters
                        .iter()
                        .filter(|p| !instance.assignments.iter().any(|a| a.name == p.name))
                        .map(|p| parameter(p, None)),
                );
            }
            object.insert("parameters".into(), Value::Array(parameters));
        }
        if included(meta, "constants")
            && (original_fields(meta).is_some() || !class.constants.is_empty())
        {
            object.insert(
                "constants".into(),
                Value::Array(
                    class
                        .constants
                        .iter()
                        .map(|declaration| parameter(declaration, None))
                        .collect(),
                ),
            );
        }
        if let Some(body) = &class.body {
            if original_fields(meta).is_none() {
                object.insert("category".into(), Value::String("composite".into()));
            }
            let mut children = Vec::new();
            for child in &body.instances {
                children.push(self.instance(child, &format!("{path}.{}", child.name))?);
            }
            if included(meta, "blocks") || !children.is_empty() {
                object.insert("blocks".into(), Value::Array(children));
            }
            if included(meta, "connections") || !body.connections.is_empty() {
                let mut wires = Vec::new();
                for wire in &body.connections {
                    let mut object = extra(&wire.meta);
                    write_metadata(&mut object, &wire.meta);
                    let unchanged = [
                        (&wire.a, "ace:compat:original_a"),
                        (&wire.b, "ace:compat:original_b"),
                    ]
                    .iter()
                    .all(|(end, key)| {
                        wire.meta
                            .annotations
                            .get(*key)
                            .and_then(|value| {
                                serde_json::from_value::<Endpoint>(json_annotation(value)).ok()
                            })
                            .as_ref()
                            == Some(*end)
                    });
                    if unchanged {
                        wires.push(Value::Object(object));
                        continue;
                    }
                    let source = |endpoint: &Endpoint| {
                        if let Some(owner) = &endpoint.instance {
                            body.instances
                                .iter()
                                .find(|i| &i.name == owner)
                                .and_then(|instance| {
                                    let class =
                                        self.document.library.classes.get(&instance.class)?;
                                    instance
                                        .connectors
                                        .iter()
                                        .find(|p| p.name == endpoint.connector)
                                        .or_else(|| class.connector(&endpoint.connector))
                                })
                                .map(|p| p.direction == cdl_model::Direction::Output)
                        } else {
                            class
                                .connector(&endpoint.connector)
                                .map(|p| p.direction == cdl_model::Direction::Input)
                        }
                    };
                    let (from, to) = match (source(&wire.a), source(&wire.b)) {
                        (Some(true), Some(false)) => (&wire.a, &wire.b),
                        (Some(false), Some(true)) => (&wire.b, &wire.a),
                        // Preserve malformed legacy wiring for the validator to diagnose.
                        _ => (&wire.a, &wire.b),
                    };
                    for (side, endpoint, block_key, port_key) in [
                        ("from", from, "from_block", "from_output"),
                        ("to", to, "to_block", "to_input"),
                    ] {
                        if included(&wire.meta, side) {
                            object.insert(side.into(), Value::String(endpoint.to_string()));
                        }
                        if included(&wire.meta, block_key) {
                            object.insert(
                                block_key.into(),
                                Value::String(
                                    endpoint
                                        .instance
                                        .clone()
                                        .unwrap_or_else(|| endpoint.connector.clone()),
                                ),
                            );
                        }
                        if included(&wire.meta, port_key) {
                            object.insert(
                                port_key.into(),
                                Value::String(if endpoint.instance.is_some() {
                                    endpoint.connector.clone()
                                } else {
                                    String::new()
                                }),
                            );
                        }
                    }
                    wires.push(Value::Object(object));
                }
                object.insert("connections".into(), Value::Array(wires));
            }
        } else {
            for field in ["blocks", "connections"] {
                if included(meta, field) {
                    object.insert(field.into(), Value::Array(Vec::new()));
                }
            }
        }
        if let Some(equations) = class.meta.annotations.get(EQUATIONS).or_else(|| {
            if original_fields(meta).is_none() || included(meta, "equations") {
                meta.annotations.get(EQUATIONS)
            } else {
                None
            }
        }) {
            object.insert("equations".into(), json_annotation(equations));
        }
        Ok(Value::Object(object))
    }
}
