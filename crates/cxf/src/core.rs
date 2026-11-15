//! Pinned modelica-json interface index shared by format adapters.
use serde_json::Value;
#[cfg(feature = "bundled-core")]
use std::sync::OnceLock;

/// Canonical ASHRAE 231 namespace.
pub const STANDARD: &str = "http://data.ashrae.org/S231#";
/// Namespace for explicitly identified Explorer extensions.
pub const ACE: &str = "https://ace-iot-solutions.github.io/ace-cdl-explorer/cxf#";

#[derive(Debug)]
/// Authoritative elementary declaration with a compact name-resolution signature.
pub struct Interface {
    /// Complete typed model interface.
    pub definition: cdl_model::BlockClass,
    /// Enumeration identities declared by the pinned core.
    pub enumerations: Vec<cdl_model::Enumeration>,
    /// Expanded standard class identifier.
    pub id: String,
    /// Local input names and primitive type spellings.
    pub inputs: Vec<(String, String)>,
    /// Local output names and primitive type spellings.
    pub outputs: Vec<(String, String)>,
    /// Local parameter names.
    pub parameters: Vec<String>,
}

pub(crate) fn refs(value: &Value) -> Vec<&str> {
    match value {
        Value::Array(values) => values.iter().filter_map(|v| v["@id"].as_str()).collect(),
        Value::Object(_) => value["@id"].as_str().into_iter().collect(),
        _ => Vec::new(),
    }
}

#[cfg(feature = "bundled-core")]
/// Cached pinned interface index, decoded from this crate's bundled resource.
pub fn interfaces() -> &'static [Interface] {
    static INTERFACES: OnceLock<Vec<Interface>> = OnceLock::new();
    INTERFACES.get_or_init(|| {
        let document: Value = serde_json::from_str(include_str!("../resources/CXF-Core.jsonld"))
            .expect("checked-in CXF-Core is JSON");
        let graph = document["@graph"].as_array().expect("CXF-Core graph");
        let index: std::collections::BTreeMap<_, _> = graph
            .iter()
            .filter_map(|n| Some((n["@id"].as_str()?, n)))
            .collect();
        graph
            .iter()
            .filter(|n| n["@type"] == "S231:ElementaryBlock")
            .map(|n| {
                let ports = |key: &str| {
                    refs(&n[key])
                        .into_iter()
                        .map(|id| {
                            let port = index[id];
                            let kind = port["@type"].as_str().unwrap_or("");
                            let kind = if kind.contains("Boolean") {
                                "Boolean"
                            } else if kind.contains("Integer") {
                                "Integer"
                            } else if kind.contains("String") {
                                "String"
                            } else {
                                "Real"
                            };
                            (id.rsplit('.').next().unwrap().to_owned(), kind.to_owned())
                        })
                        .collect()
                };
                Interface {
                    definition: definition(n, &index),
                    enumerations: index
                        .values()
                        .filter(|node| node["@type"] == "S231:EnumerationType")
                        .map(|node| cdl_model::Enumeration {
                            id: node["@id"].as_str().unwrap().into(),
                            members: Vec::new(),
                            meta: meta(node),
                        })
                        .collect(),
                    id: n["@id"]
                        .as_str()
                        .unwrap()
                        .replace("http://example.org#", STANDARD),
                    inputs: ports("S231:hasInput"),
                    outputs: ports("S231:hasOutput"),
                    parameters: refs(&n["S231:hasParameter"])
                        .into_iter()
                        .map(|id| id.rsplit('.').next().unwrap().to_owned())
                        .collect(),
                }
            })
            .collect()
    })
}

pub(crate) fn definition(
    node: &Value,
    index: &std::collections::BTreeMap<&str, &Value>,
) -> cdl_model::BlockClass {
    use cdl_model::*;
    let mut class = BlockClass::new(
        node["@id"]
            .as_str()
            .unwrap()
            .replace("http://example.org#", STANDARD),
        BlockKind::Elementary,
    );
    for (field, direction) in [
        ("S231:hasInput", Direction::Input),
        ("S231:hasOutput", Direction::Output),
    ] {
        for id in refs(&node[field]) {
            let source = index[id];
            let mut port = Connector::new(
                id.rsplit('.').next().unwrap(),
                direction,
                data_type(source["@type"].as_str().unwrap_or("Real")),
            );
            port.units = units(source);
            port.dimensions =
                crate::format::dimensions(source.as_object().expect("indexed core node"));
            port.required = direction == Direction::Input;
            port.meta = meta(source);
            if direction == Direction::Input {
                class.inputs.push(port);
            } else {
                class.outputs.push(port);
            }
        }
    }
    for (field, constant) in [("S231:hasParameter", false), ("S231:hasConstant", true)] {
        for id in refs(&node[field]) {
            let source = index[id];
            let declaration = ParameterDecl {
                name: id.rsplit('.').next().unwrap().into(),
                data_type: source["S231:isOfDataType"]["@id"]
                    .as_str()
                    .map(data_type)
                    .unwrap_or_else(|| {
                        let expression = source["S231:value"].as_str().unwrap_or("");
                        let candidate = expression
                            .rsplit_once('.')
                            .map(|(class, _)| format!("{STANDARD}{class}"));
                        candidate
                            .filter(|id| {
                                index
                                    .get(id.as_str())
                                    .is_some_and(|node| node["@type"] == "S231:EnumerationType")
                            })
                            .map(|id| DataType::Enumeration(id.into()))
                            .unwrap_or(DataType::Real)
                    }),
                // Expressions, including typed JSON-LD numeric strings, are preserved here.
                // WP5's CXF reader supplies their literal/expression distinction.
                default: source.get("S231:value").map(preserved_value),
                units: units(source),
                min: source.get("S231:min").map(preserved_value),
                max: source.get("S231:max").map(preserved_value),
                dimensions: crate::format::dimensions(
                    source.as_object().expect("indexed core node"),
                ),
                meta: meta(source),
            };
            if constant {
                class.constants.push(declaration);
            } else {
                class.parameters.push(declaration);
            }
        }
    }
    class.breaks_algebraic_loop =
        matches!(class.id.0.rsplit('.').next(), Some("UnitDelay" | "Pre"));
    class.meta = meta(node);
    class
}
fn data_type(kind: &str) -> cdl_model::DataType {
    use cdl_model::DataType;
    if kind.contains("Boolean") {
        DataType::Boolean
    } else if kind.contains("Integer") {
        DataType::Integer
    } else if kind.contains("String") {
        DataType::String
    } else {
        DataType::Real
    }
}
fn preserved_value(value: &Value) -> cdl_model::Value {
    use cdl_model::Value as ModelValue;
    match value {
        Value::Bool(value) => ModelValue::Boolean(*value),
        Value::Number(value) => value
            .as_i64()
            .map(ModelValue::Integer)
            .unwrap_or_else(|| ModelValue::Real(value.as_f64().unwrap())),
        Value::String(value) => ModelValue::Expr(value.clone()),
        Value::Array(value) => ModelValue::Array(value.iter().map(preserved_value).collect()),
        Value::Object(object) if object.contains_key("@value") => {
            let raw = &object["@value"];
            if let Some(text) = raw.as_str() {
                let kind = object.get("@type").and_then(Value::as_str).unwrap_or("");
                if kind.ends_with("#boolean") {
                    return text
                        .parse::<bool>()
                        .map(ModelValue::Boolean)
                        .unwrap_or_else(|_| ModelValue::Expr(text.into()));
                }
                if kind.ends_with("#string") {
                    return ModelValue::String(text.into());
                }
                if kind.ends_with("#integer") {
                    return text
                        .parse::<i64>()
                        .map(ModelValue::Integer)
                        .unwrap_or_else(|_| ModelValue::Expr(text.into()));
                }
                text.parse::<f64>()
                    .map(ModelValue::Real)
                    .unwrap_or_else(|_| ModelValue::Expr(text.into()))
            } else {
                preserved_value(raw)
            }
        }
        _ => ModelValue::Expr(value.to_string()),
    }
}
fn units(node: &Value) -> cdl_model::Units {
    let expand = |id: &str| {
        id.replace("unit:", "http://qudt.org/vocab/unit#")
            .replace("q:", "http://qudt.org/vocab/quantitykind#")
    };
    cdl_model::Units {
        unit: refs(&node["qudt:hasUnit"]).first().map(|id| expand(id)),
        display_unit: refs(&node["S231:hasDisplayUnit"])
            .first()
            .map(|id| expand(id)),
        quantity_kind: refs(&node["qudt:hasQuantityKind"])
            .first()
            .map(|id| expand(id)),
    }
}
pub(crate) fn meta(node: &Value) -> cdl_model::Meta {
    cdl_model::Meta {
        label: node["S231:label"].as_str().map(str::to_owned),
        description: node["S231:description"].as_str().map(str::to_owned),
        annotations: cdl_model::IndexMap::new(),
    }
}
