use cdl_model::{DataType, Diagnostic, DiagnosticCode, Dimension, Meta, Units, Value as Scalar};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
pub(crate) const S: &str = "http://data.ashrae.org/S231#";
pub(crate) const ACE: &str = "https://ace-iot-solutions.github.io/ace-cdl-explorer/cxf#";
pub(crate) fn error(path: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(path, DiagnosticCode::InvalidDocument, message)
}
pub(crate) fn name(id: &str) -> String {
    let raw = id.rsplit(['.', '#']).next().unwrap_or(id);
    let mut bytes = Vec::new();
    let input = raw.as_bytes();
    let mut i = 0;
    while i < input.len() {
        if input[i] == b'%' && i + 2 < input.len() {
            if let (Some(a), Some(b)) = (
                (input[i + 1] as char).to_digit(16),
                (input[i + 2] as char).to_digit(16),
            ) {
                bytes.push((a * 16 + b) as u8);
                i += 3;
                continue;
            }
        }
        bytes.push(input[i]);
        i += 1;
    }
    String::from_utf8(bytes).unwrap_or_else(|_| raw.into())
}

pub(crate) fn class_id(id: &str) -> String {
    id.strip_prefix("http://example.org#")
        .map(|s| format!("{S}{s}"))
        .unwrap_or_else(|| id.into())
}
pub(crate) fn refs(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(values)) => values.iter().flat_map(|v| refs(Some(v))).collect(),
        Some(Value::Object(v)) => v
            .get("@id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .into_iter()
            .collect(),
        _ => vec![],
    }
}
pub(crate) fn reference(id: impl Into<String>) -> Value {
    serde_json::json!({"@id":id.into()})
}
pub(crate) fn literal(value: &Value) -> &Value {
    value.get("@value").unwrap_or(value)
}
fn unsigned(value: &Value) -> Option<u64> {
    let v = literal(value);
    v.as_u64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
}
fn boolean(value: &Value) -> Option<bool> {
    let v = literal(value);
    v.as_bool()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
}
pub(crate) fn text(value: Option<&Value>) -> Option<String> {
    value.and_then(|v| literal(v).as_str()).map(str::to_owned)
}
pub(crate) fn dtype(value: &str) -> DataType {
    let value = class_id(value);
    let local = value.rsplit(['#', ':']).next().unwrap_or(&value);
    if local.starts_with("Boolean") {
        DataType::Boolean
    } else if local.starts_with("Integer") {
        DataType::Integer
    } else if local.starts_with("String") {
        DataType::String
    } else if local.starts_with("Real") {
        DataType::Real
    } else {
        DataType::Enumeration(value.into())
    }
}
pub(crate) fn type_name(kind: &DataType) -> &str {
    match kind {
        DataType::Real => "Real",
        DataType::Integer => "Integer",
        DataType::Boolean => "Boolean",
        DataType::String => "String",
        DataType::Enumeration(_) => "Enumeration",
    }
}
pub(crate) fn scalar(value: &Value, kind: &DataType) -> Scalar {
    let raw = literal(value);
    match raw {
        Value::Bool(v) => Scalar::Boolean(*v),
        Value::Number(v) => v
            .as_i64()
            .map(Scalar::Integer)
            .unwrap_or_else(|| Scalar::Real(v.as_f64().unwrap())),
        Value::Array(v) => Scalar::Array(v.iter().map(|v| scalar(v, kind)).collect()),
        Value::String(s) => {
            let tag = value.get("@type").and_then(Value::as_str).unwrap_or("");
            if tag == "ace:Expression" || tag == format!("{ACE}Expression") {
                Scalar::Expr(s.clone())
            } else if tag == "ace:EnumerationMember" || tag == format!("{ACE}EnumerationMember") {
                Scalar::Enum(s.clone())
            } else if tag.ends_with("#string") || *kind == DataType::String {
                Scalar::String(s.clone())
            } else if tag.ends_with("#boolean") {
                s.parse()
                    .map(Scalar::Boolean)
                    .unwrap_or_else(|_| Scalar::Expr(s.clone()))
            } else if tag.ends_with("#integer") {
                s.parse()
                    .map(Scalar::Integer)
                    .unwrap_or_else(|_| Scalar::Expr(s.clone()))
            } else if ["#decimal", "#double", "#float"]
                .iter()
                .any(|t| tag.ends_with(t))
            {
                s.parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite())
                    .map(Scalar::Real)
                    .unwrap_or_else(|| Scalar::Expr(s.clone()))
            } else {
                Scalar::Expr(s.clone())
            }
        }
        _ => Scalar::Expr(raw.to_string()),
    }
}
pub(crate) fn scalar_json(value: &Scalar, kind: &DataType) -> Value {
    match value {
        Scalar::Real(v) => serde_json::json!(v),
        Scalar::Integer(v) => serde_json::json!(v),
        Scalar::Boolean(v) => serde_json::json!(v),
        Scalar::String(v) => {
            serde_json::json!({"@value":v,"@type":"http://www.w3.org/2001/XMLSchema#string"})
        }
        Scalar::Expr(v) if *kind == DataType::String => {
            serde_json::json!({"@value":v,"@type":format!("{ACE}Expression")})
        }
        Scalar::Expr(v) => Value::String(v.clone()),
        Scalar::Enum(v) => {
            serde_json::json!({"@value":v,"@type":format!("{ACE}EnumerationMember")})
        }
        Scalar::Array(v) => Value::Array(v.iter().map(|v| scalar_json(v, kind)).collect()),
    }
}
pub(crate) fn units(node: &Map<String, Value>) -> Units {
    Units {
        unit: refs(node.get("qudt:hasUnit")).into_iter().next(),
        display_unit: refs(node.get("S231:hasDisplayUnit")).into_iter().next(),
        quantity_kind: refs(node.get("qudt:hasQuantityKind")).into_iter().next(),
    }
}
fn dimension_parts(raw: &str) -> Vec<&str> {
    fn separators(s: &str) -> Option<Vec<usize>> {
        let mut stack = Vec::new();
        let mut quote = None;
        let mut escaped = false;
        let mut commas = Vec::new();
        for (i, c) in s.char_indices() {
            if let Some(q) = quote {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == q {
                    quote = None;
                }
                continue;
            }
            if c == '\'' || c == '"' {
                quote = Some(c);
                continue;
            }
            match c {
                '(' | '[' | '{' => stack.push(c),
                ')' | ']' | '}' => {
                    let expected = match c {
                        ')' => '(',
                        ']' => '[',
                        _ => '{',
                    };
                    if stack.pop() != Some(expected) {
                        return None;
                    }
                }
                ',' if stack.is_empty() => commas.push(i),
                _ => {}
            }
        }
        (stack.is_empty() && quote.is_none()).then_some(commas)
    }
    let raw = raw.trim();
    let inner = raw
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .filter(|s| separators(s).is_some())
        .unwrap_or(raw);
    let Some(commas) = separators(inner) else {
        return vec![raw];
    };
    let mut parts = Vec::new();
    let mut start = 0;
    for i in commas {
        parts.push(inner[start..i].trim());
        start = i + 1;
    }
    parts.push(inner[start..].trim());
    parts.into_iter().filter(|s| !s.is_empty()).collect()
}
pub(crate) fn dimensions(node: &Map<String, Value>) -> Vec<Dimension> {
    match node.get("S231:sizeOfDimensions").map(literal) {
        Some(Value::Array(values)) => values
            .iter()
            .map(|v| {
                unsigned(v).map(Dimension::Size).unwrap_or_else(|| {
                    Dimension::Expr(
                        literal(v)
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| v.to_string()),
                    )
                })
            })
            .collect(),
        Some(Value::String(v)) => dimension_parts(v)
            .into_iter()
            .map(|s| {
                s.parse()
                    .map(Dimension::Size)
                    .unwrap_or_else(|_| Dimension::Expr(s.into()))
            })
            .collect(),
        Some(Value::Number(v)) if v.as_u64().is_some() => {
            vec![Dimension::Size(v.as_u64().unwrap())]
        }
        _ if node
            .get("S231:isArray")
            .is_some_and(|v| boolean(v) == Some(true)) =>
        {
            let count = node
                .get("S231:numberDimensions")
                .and_then(unsigned)
                .unwrap_or(1);
            (0..count).map(|_| Dimension::Expr(":".into())).collect()
        }
        _ => vec![],
    }
}
pub(crate) fn validate_array_rank(node: &Map<String, Value>) -> Result<(), Diagnostic> {
    if node
        .get("S231:isArray")
        .is_some_and(|v| boolean(v) == Some(true))
        && !node.contains_key("S231:numberDimensions")
        && !node.contains_key("S231:sizeOfDimensions")
    {
        return Err(error(
            node.get("@id").and_then(Value::as_str).unwrap_or(""),
            "array rank or extents are required",
        ));
    }
    if let Some(raw) = node.get("S231:numberDimensions") {
        let rank = unsigned(raw).ok_or_else(|| {
            error(
                node.get("@id").and_then(Value::as_str).unwrap_or(""),
                "array rank must be a nonnegative integer",
            )
        })?;
        if rank > 1024
            || rank == 0
                && node
                    .get("S231:isArray")
                    .is_some_and(|v| boolean(v) == Some(true))
        {
            return Err(error(
                node.get("@id").and_then(Value::as_str).unwrap_or(""),
                "array rank must be between 1 and 1024",
            ));
        }
        if node.contains_key("S231:sizeOfDimensions") && dimensions(node).len() as u64 != rank {
            return Err(error(
                node.get("@id").and_then(Value::as_str).unwrap_or(""),
                "array rank and extent count disagree",
            ));
        }
    }
    Ok(())
}
pub(crate) fn metadata(node: &Map<String, Value>, known: &[&str]) -> Meta {
    let mut meta = Meta {
        label: text(node.get("S231:label")),
        description: text(node.get("S231:description")),
        ..Meta::default()
    };
    for (key, value) in node {
        if key.contains(':')
            && !known.contains(&key.as_str())
            && !matches!(
                key.as_str(),
                "S231:label" | "S231:description" | "S231:isConnectedTo" | "S231:connectedTo"
            )
        {
            meta.annotations.insert(
                key.clone(),
                serde_json::from_value(value.clone()).expect("JSON annotation"),
            );
        }
    }
    meta
}
pub(crate) fn write_meta(node: &mut Map<String, Value>, meta: &Meta) {
    if let Some(v) = &meta.label {
        node.insert("S231:label".into(), Value::String(v.clone()));
    }
    if let Some(v) = &meta.description {
        node.insert("S231:description".into(), Value::String(v.clone()));
    }
    for (key, value) in &meta.annotations {
        if !key.starts_with("ace:compat:") {
            node.insert(
                key.clone(),
                serde_json::to_value(value).expect("annotation JSON"),
            );
        }
    }
}
pub(crate) fn write_units(node: &mut Map<String, Value>, units: &Units) {
    for (key, value) in [
        ("qudt:hasUnit", &units.unit),
        ("S231:hasDisplayUnit", &units.display_unit),
        ("qudt:hasQuantityKind", &units.quantity_kind),
    ] {
        if let Some(value) = value {
            node.insert(key.into(), reference(value));
        }
    }
}
pub(crate) fn write_dimensions(node: &mut Map<String, Value>, dimensions: &[Dimension]) {
    if !dimensions.is_empty() {
        node.insert("S231:isArray".into(), Value::Bool(true));
        node.insert(
            "S231:numberDimensions".into(),
            serde_json::json!(dimensions.len()),
        );
        node.insert(
            "S231:sizeOfDimensions".into(),
            Value::Array(
                dimensions
                    .iter()
                    .map(|d| match d {
                        Dimension::Size(v) => serde_json::json!(v),
                        Dimension::Expr(v) => Value::String(v.clone()),
                    })
                    .collect(),
            ),
        );
    }
}
/// The supported compact-context subset. No remote document loader is involved.
pub(crate) struct Index {
    pub nodes: BTreeMap<String, Map<String, Value>>,
    pub order: Vec<String>,
    pub meta: Map<String, Value>,
}
impl Index {
    pub fn parse(source: &Value) -> Result<Self, Diagnostic> {
        let mut prefixes = BTreeMap::from_iter([
            ("S231".to_string(), S.to_string()),
            ("ace".into(), ACE.into()),
            ("unit".into(), "http://qudt.org/vocab/unit#".into()),
            ("q".into(), "http://qudt.org/vocab/quantitykind#".into()),
            ("qudt".into(), "http://qudt.org/schema/qudt/".into()),
        ]);
        if let Some(context) = source.get("@context") {
            let Value::Object(context) = context else {
                return Err(error(
                    "@context",
                    "only inline prefix contexts are supported",
                ));
            };
            for (key, value) in context {
                if key.starts_with('@')
                    || value
                        .get("@prefix")
                        .is_some_and(|v| v != &Value::Bool(true))
                    || value
                        .as_object()
                        .is_some_and(|v| v.keys().any(|k| k != "@id" && k != "@prefix"))
                {
                    return Err(error(
                        "@context",
                        format!("unsupported non-prefix context term {key}"),
                    ));
                }
                let iri = value
                    .as_str()
                    .or_else(|| value.get("@id").and_then(Value::as_str))
                    .ok_or_else(|| error("@context", format!("unsupported context term {key}")))?;
                if !iri.contains(':') || !iri.ends_with(['#', '/', ':']) {
                    return Err(error(
                        "@context",
                        format!("expected an absolute namespace prefix for {key}"),
                    ));
                }
                prefixes.insert(key.clone(), iri.into());
            }
        }
        fn expand(s: &str, p: &BTreeMap<String, String>) -> String {
            if let Some((prefix, local)) = s.split_once(':') {
                if let Some(base) = p.get(prefix) {
                    return format!("{base}{local}");
                }
            }
            s.into()
        }
        fn key(s: &str, p: &BTreeMap<String, String>) -> String {
            let e = expand(s, p);
            for (base, prefix) in [
                (S, "S231:"),
                (ACE, "ace:"),
                ("http://qudt.org/schema/qudt/", "qudt:"),
                ("http://qudt.org/schema/qudt#", "qudt:"),
            ] {
                if let Some(local) = e.strip_prefix(base) {
                    return format!("{prefix}{local}");
                }
            }
            e
        }
        fn normalize(v: &Value, p: &BTreeMap<String, String>) -> Value {
            match v {
                Value::Array(a) => Value::Array(a.iter().map(|v| normalize(v, p)).collect()),
                Value::Object(o) => Value::Object(
                    o.iter()
                        .map(|(k, v)| {
                            let v = if k == "@id" {
                                v.as_str()
                                    .map(|s| Value::String(expand(s, p)))
                                    .unwrap_or_else(|| v.clone())
                            } else if k == "@type" {
                                match v {
                                    Value::String(s) => Value::String(key(s, p)),
                                    Value::Array(a) => Value::Array(
                                        a.iter()
                                            .map(|v| {
                                                v.as_str()
                                                    .map(|s| Value::String(key(s, p)))
                                                    .unwrap_or_else(|| v.clone())
                                            })
                                            .collect(),
                                    ),
                                    _ => v.clone(),
                                }
                            } else {
                                normalize(v, p)
                            };
                            (key(k, p), v)
                        })
                        .collect(),
                ),
                _ => v.clone(),
            }
        }
        let graph = source
            .get("@graph")
            .and_then(Value::as_array)
            .ok_or_else(|| error("@graph", "expected a CXF @graph array"))?;
        let mut nodes = BTreeMap::new();
        let mut order = Vec::new();
        for node in graph {
            if node.get("@context").is_some() {
                return Err(error("@graph", "scoped node contexts are not supported"));
            }
            let Value::Object(node) = normalize(node, &prefixes) else {
                return Err(error("@graph", "expected node object"));
            };
            if node.keys().any(|key| key.starts_with("ace:compat:")) {
                return Err(error(
                    "@graph",
                    "reserved compatibility metadata is not accepted from CXF",
                ));
            }
            let id = node
                .get("@id")
                .and_then(Value::as_str)
                .ok_or_else(|| error("@graph", "expected string @id"))?
                .to_owned();
            validate_array_rank(&node)?;
            if !id.contains(':') || id.chars().any(char::is_whitespace) {
                return Err(error(&id, "expected an expanded absolute node IRI"));
            }
            for field in [
                "S231:containsBlock",
                "S231:hasInput",
                "S231:hasOutput",
                "S231:hasParameter",
                "S231:hasConstant",
                "S231:hasInstance",
                "S231:isConnectedTo",
                "S231:connectedTo",
                "S231:isOfDataType",
                "qudt:hasUnit",
                "qudt:hasQuantityKind",
                "S231:hasDisplayUnit",
                "ace:connections",
            ] {
                if let Some(value) = node.get(field).filter(|v| !v.is_null()) {
                    let records = match value {
                        Value::Array(v) => v.iter().collect::<Vec<_>>(),
                        Value::Object(_) => vec![value],
                        _ => return Err(error(&id, format!("{field} requires @id references"))),
                    };
                    if records
                        .iter()
                        .any(|r| r.get("@id").and_then(Value::as_str).is_none())
                    {
                        return Err(error(
                            &id,
                            format!("{field} requires string @id references"),
                        ));
                    }
                }
            }
            if nodes.insert(id.clone(), node).is_some() {
                return Err(error(&id, "duplicate @id"));
            }
            order.push(id);
        }
        let meta: Map<String, Value> = source
            .as_object()
            .unwrap()
            .iter()
            .filter(|(k, _)| !k.starts_with('@'))
            .map(|(k, v)| (key(k, &prefixes), normalize(v, &prefixes)))
            .collect();
        if meta.keys().any(|key| key.starts_with("ace:compat:")) {
            return Err(error(
                "@graph",
                "reserved compatibility metadata is not accepted from CXF",
            ));
        }
        Ok(Self { nodes, order, meta })
    }
}
