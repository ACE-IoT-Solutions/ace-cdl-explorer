//! Worked model construction for the pinned CoolingCoil fixture (CXF §8.2/8.5).
//! This demonstrates model values and references; it is not a general CXF reader.
use cdl_model::*;
use serde_json::Value as Json;
use std::collections::HashMap;

const STANDARD: &str = "http://data.ashrae.org/S231#";
fn iri(id: &str) -> String {
    id.replace("http://example.org#", STANDARD)
        .replace("S231:", STANDARD)
        .replace("unit:", "http://qudt.org/vocab/unit#")
        .replace("q:", "http://qudt.org/vocab/quantitykind#")
}
fn refs(node: &Json, key: &str) -> Vec<String> {
    let value = &node[key];
    match value {
        Json::Array(values) => values
            .iter()
            .filter_map(|v| v["@id"].as_str().map(str::to_owned))
            .collect(),
        Json::Object(_) => value["@id"]
            .as_str()
            .map(str::to_owned)
            .into_iter()
            .collect(),
        _ => vec![],
    }
}
fn local(id: &str) -> &str {
    id.rsplit(['.', '#']).next().unwrap()
}
fn kind(value: &str) -> DataType {
    if value.contains("Boolean") {
        DataType::Boolean
    } else if value.contains("Integer") {
        DataType::Integer
    } else if value.contains("String") {
        DataType::String
    } else {
        DataType::Real
    }
}
fn value(raw: &Json) -> Value {
    let literal = raw.get("@value").unwrap_or(raw);
    match literal {
        Json::Bool(v) => Value::Boolean(*v),
        Json::Number(v) => v
            .as_i64()
            .map(Value::Integer)
            .unwrap_or_else(|| Value::Real(v.as_f64().unwrap())),
        Json::String(v) if raw.get("@type").is_some() => v
            .parse::<f64>()
            .map(Value::Real)
            .unwrap_or_else(|_| Value::Expr(v.clone())),
        Json::String(v) => Value::Expr(v.clone()),
        _ => Value::Expr(literal.to_string()),
    }
}
fn units(node: &Json) -> Units {
    Units {
        unit: refs(node, "qudt:hasUnit").first().map(|s| iri(s)),
        display_unit: refs(node, "S231:hasDisplayUnit").first().map(|s| iri(s)),
        quantity_kind: refs(node, "qudt:hasQuantityKind").first().map(|s| iri(s)),
    }
}
fn class(node: &Json, index: &HashMap<String, Json>, composite: bool) -> BlockClass {
    let mut class = BlockClass::new(
        iri(node["@id"].as_str().unwrap()),
        if composite {
            BlockKind::Composite
        } else {
            BlockKind::Elementary
        },
    );
    for (key, direction) in [
        ("S231:hasInput", Direction::Input),
        ("S231:hasOutput", Direction::Output),
    ] {
        for id in refs(node, key) {
            let source = &index[&id];
            let mut connector = Connector::new(
                local(&id),
                direction,
                kind(source["@type"].as_str().unwrap_or("Real")),
            );
            connector.units = units(source);
            if direction == Direction::Input {
                class.inputs.push(connector);
            } else {
                class.outputs.push(connector);
            }
        }
    }
    for id in refs(node, "S231:hasParameter") {
        let source = &index[&id];
        class.parameters.push(ParameterDecl {
            name: local(&id).into(),
            data_type: kind(
                source["S231:isOfDataType"]["@id"]
                    .as_str()
                    .unwrap_or("Real"),
            ),
            default: source.get("S231:value").map(value),
            units: units(source),
            min: source.get("S231:min").map(value),
            max: source.get("S231:max").map(value),
            dimensions: vec![],
            meta: Meta::default(),
        });
    }
    class
}
fn build() -> Document {
    let fixture: Json = serde_json::from_str(include_str!(
        "../../../web/tests/fixtures/CoolingCoil.jsonld"
    ))
    .unwrap();
    let core: Json =
        serde_json::from_str(include_str!("../../../web/public/schemas/CXF-Core.jsonld")).unwrap();
    let index: HashMap<String, Json> = core["@graph"]
        .as_array()
        .unwrap()
        .iter()
        .chain(fixture["@graph"].as_array().unwrap())
        .map(|node| (node["@id"].as_str().unwrap().to_owned(), node.clone()))
        .collect();
    let source = &fixture["@graph"][0];
    let root_id = source["@id"].as_str().unwrap();
    let mut root = class(source, &index, true);
    let mut library = Library::default();
    let mut children = Vec::new();
    for id in refs(source, "S231:containsBlock") {
        let node = &index[&id];
        let class_id = iri(node["@type"].as_str().unwrap());
        let definition = class(&index[&class_id], &index, false);
        let mut instance = Instance::new(local(&id), class_id.clone());
        for parameter in &definition.parameters {
            if let Some(binding) = index
                .get(&format!("{id}.{}", parameter.name))
                .and_then(|n| n.get("S231:value"))
            {
                instance.assignments.push(Assignment {
                    name: parameter.name.clone(),
                    value: value(binding),
                    meta: Meta::default(),
                });
            }
        }
        library.classes.insert(definition.id.clone(), definition);
        children.push(instance);
    }
    let endpoint = |id: &str| Endpoint::parse(id.strip_prefix(&format!("{root_id}.")).unwrap());
    let mut connections = Vec::new();
    for node in fixture["@graph"].as_array().unwrap() {
        for to in refs(node, "S231:isConnectedTo")
            .into_iter()
            .chain(refs(node, "S231:connectedTo"))
        {
            connections.push(Connection {
                a: endpoint(node["@id"].as_str().unwrap()),
                b: endpoint(&to),
                meta: Meta::default(),
            });
        }
    }
    root.body = Some(CompositeBody {
        instances: children,
        connections,
    });
    let root_instance = Instance::new("CoolingCoil", root.id.clone());
    library.classes.insert(root.id.clone(), root);
    Document {
        library,
        root: root_instance,
    }
}
fn main() {
    let document = build();
    let diagnostics = validate(&document);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let root = &document.library.classes[&document.root.class];
    assert_eq!(root.body.as_ref().unwrap().instances.len(), 6);
    assert_eq!(root.inputs.len(), 4);
    assert_eq!(root.outputs.len(), 1);
    let instances = document.instances().unwrap();
    let controller = instances
        .iter()
        .find(|i| i.instance.name == "conCoi")
        .unwrap();
    assert_eq!(
        controller
            .instance
            .assignments
            .iter()
            .find(|p| p.name == "k")
            .unwrap()
            .value,
        Value::Expr("kCooCoi".into())
    );
    assert_eq!(
        controller
            .instance
            .assignments
            .iter()
            .find(|p| p.name == "reverseActing")
            .unwrap()
            .value,
        Value::Boolean(false)
    );
    println!(
        "CoolingCoil: {} child instances, {} wires, structurally valid",
        instances.len() - 1,
        root.body.as_ref().unwrap().connections.len()
    );
}
