use crate::format::*;
use cdl_model::*;
use serde_json::{Map, Value as Json};
use std::collections::BTreeMap;

/// Export a canonical document as compacted CXF JSON-LD, without redefining standard classes.
pub fn to_json(document: &Document) -> Result<String, Vec<Diagnostic>> {
    serde_json::to_string_pretty(&to_value(document)?).map_err(|e| vec![error("", e.to_string())])
}
/// Export class templates, configured instances and local connector references.
pub fn to_value(document: &Document) -> Result<Json, Vec<Diagnostic>> {
    write(document).map_err(|e| vec![e])
}
fn encode(name: &str) -> String {
    name.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
fn node(id: &str, kind: &str) -> Map<String, Json> {
    Map::from_iter([
        ("@id".into(), Json::String(id.into())),
        ("@type".into(), Json::String(kind.into())),
    ])
}
fn datatype(kind: &DataType) -> Json {
    match kind {
        DataType::Enumeration(id) => reference(&id.0),
        _ => reference(format!("{S}{}", type_name(kind))),
    }
}
fn write(document: &Document) -> Result<Json, Diagnostic> {
    document
        .instances()
        .map_err(|e| e.into_iter().next().unwrap())?;
    let mut writer = Writer {
        document,
        nodes: BTreeMap::new(),
        order: Vec::new(),
    };
    for class in document.library.classes.values() {
        if !(class.kind == BlockKind::Elementary
            && class
                .id
                .0
                .starts_with(&format!("{S}Buildings.Controls.OBC.CDL.")))
        {
            writer.class(class)?;
        }
    }
    for e in document.library.enumerations.values() {
        if !e
            .id
            .0
            .starts_with(&format!("{S}Buildings.Controls.OBC.CDL.Types."))
        {
            let mut record = node(&e.id.0, "S231:EnumerationType");
            write_meta(&mut record, &e.meta);
            record.insert(
                "ace:enumerationMembers".into(),
                serde_json::json!(e.members),
            );
            writer.add(&e.id.0, record)?;
        }
    }
    let mut root = format!("{ACE}root.{}", encode(&document.root.name));
    while writer.nodes.contains_key(&root) || document.library.classes.contains_key(root.as_str()) {
        root.push('~');
    }
    writer.instance(&root, &document.root)?;
    // Root first makes output convenient for human readers without depending on order.
    let mut graph = vec![Json::Object(writer.nodes.remove(&root).unwrap())];
    for id in writer.order {
        if let Some(node) = writer.nodes.remove(&id) {
            graph.push(Json::Object(node));
        }
    }
    let mut output = Map::new();
    write_meta(&mut output, &document.library.meta);
    output.insert("@context".into(),serde_json::json!({"S231":S,"ace":ACE,"qudt":"http://qudt.org/schema/qudt/","unit":"http://qudt.org/vocab/unit#","q":"http://qudt.org/vocab/quantitykind#"}));
    output.insert("ace:rootBlock".into(), reference(root));
    output.insert("@graph".into(), Json::Array(graph));
    Ok(Json::Object(output))
}
struct Writer<'a> {
    document: &'a Document,
    nodes: BTreeMap<String, Map<String, Json>>,
    order: Vec<String>,
}
impl Writer<'_> {
    fn add(&mut self, id: &str, node: Map<String, Json>) -> Result<(), Diagnostic> {
        if self.nodes.contains_key(id) {
            return Err(error(id, "CXF identifier collision"));
        }
        self.nodes.insert(id.into(), node);
        self.order.push(id.into());
        Ok(())
    }
    fn port(&mut self, id: &str, port: &Connector) -> Result<(), Diagnostic> {
        let direction = if port.direction == Direction::Input {
            "Input"
        } else {
            "Output"
        };
        let namespace = if matches!(port.data_type, DataType::Enumeration(_)) {
            "ace:"
        } else {
            "S231:"
        };
        let mut record = node(
            id,
            &format!("{namespace}{}{direction}", type_name(&port.data_type)),
        );
        if matches!(port.data_type, DataType::Enumeration(_)) {
            record.insert("S231:isOfDataType".into(), datatype(&port.data_type));
        }
        write_meta(&mut record, &port.meta);
        write_units(&mut record, &port.units);
        write_dimensions(&mut record, &port.dimensions);
        record.insert("ace:required".into(), Json::Bool(port.required));
        self.add(id, record)
    }
    fn parameter(
        &mut self,
        id: &str,
        p: &ParameterDecl,
        binding: Option<&Assignment>,
        constant: bool,
    ) -> Result<(), Diagnostic> {
        let mut record = node(
            id,
            if constant {
                "S231:Constant"
            } else {
                "S231:Parameter"
            },
        );
        write_meta(&mut record, &p.meta);
        write_units(&mut record, &p.units);
        write_dimensions(&mut record, &p.dimensions);

        record.insert("S231:isOfDataType".into(), datatype(&p.data_type));
        for (key, value) in [("S231:min", p.min.as_ref()), ("S231:max", p.max.as_ref())] {
            if let Some(value) = value {
                record.insert(key.into(), scalar_json(value, &p.data_type));
            }
        }
        if let Some(binding) = binding {
            write_meta(&mut record, &binding.meta);
            record.insert(
                "S231:value".into(),
                scalar_json(&binding.value, &p.data_type),
            );
        } else if let Some(value) = &p.default {
            record.insert("S231:value".into(), scalar_json(value, &p.data_type));
        }
        self.add(id, record)
    }
    fn interface(
        &mut self,
        id: &str,
        class: &BlockClass,
        record: &mut Map<String, Json>,
    ) -> Result<(), Diagnostic> {
        for (key, ports) in [
            ("S231:hasInput", &class.inputs),
            ("S231:hasOutput", &class.outputs),
        ] {
            let mut references = Vec::new();
            for port in ports {
                let port_id = format!("{id}.{}", encode(&port.name));
                self.port(&port_id, port)?;
                references.push(reference(&port_id));
            }
            if !references.is_empty() {
                record.insert(key.into(), Json::Array(references));
            }
        }
        for (key, parameters) in [
            ("S231:hasParameter", &class.parameters),
            ("S231:hasConstant", &class.constants),
        ] {
            let mut references = Vec::new();
            for parameter in parameters {
                let p_id = format!("{id}.{}", encode(&parameter.name));
                self.parameter(&p_id, parameter, None, key == "S231:hasConstant")?;
                references.push(reference(&p_id));
            }
            if !references.is_empty() {
                record.insert(key.into(), Json::Array(references));
            }
        }
        Ok(())
    }
    fn class(&mut self, class: &BlockClass) -> Result<(), Diagnostic> {
        let id = &class.id.0;
        let declared_kind = if class.meta.annotations.get("ace:interfaceOnly")
            == Some(&Annotation::Boolean(true))
        {
            "S231:CompositeBlock"
        } else {
            match class.kind {
                BlockKind::Composite => "S231:CompositeBlock",
                BlockKind::Elementary => "S231:ElementaryBlock",
                BlockKind::Extension => "S231:ExtensionBlock",
            }
        };
        let mut record = node(id, declared_kind);
        write_meta(&mut record, &class.meta);
        self.interface(id, class, &mut record)?;
        if let Some(body) = &class.body {
            let mut ids = BTreeMap::new();
            let mut children = Vec::new();
            for child in &body.instances {
                let child_id = format!("{id}.instance.{}", encode(&child.name));
                self.instance(&child_id, child)?;
                ids.insert(child.name.clone(), child_id.clone());
                children.push(reference(child_id));
            }
            record.insert("S231:containsBlock".into(), Json::Array(children));
            let mut annotated = Vec::new();
            for (index, wire) in body.connections.iter().enumerate() {
                let endpoint = |e: &Endpoint| -> Result<String, Diagnostic> {
                    Ok(format!(
                        "{}.{}",
                        if let Some(child) = &e.instance {
                            ids.get(child)
                                .ok_or_else(|| error(id, format!("missing child {child}")))?
                                .as_str()
                        } else {
                            id.as_str()
                        },
                        encode(&e.connector)
                    ))
                };
                let a = endpoint(&wire.a)?;
                let b = endpoint(&wire.b)?;
                if !self.nodes.contains_key(&b) {
                    return Err(error(&b, "missing connector while exporting wire"));
                }
                let source = self
                    .nodes
                    .get_mut(&a)
                    .ok_or_else(|| error(&a, "missing connector while exporting wire"))?;
                let links = source
                    .entry("S231:isConnectedTo")
                    .or_insert_with(|| Json::Array(vec![]));
                let links = links
                    .as_array_mut()
                    .ok_or_else(|| error(&a, "wire annotation conflicts with connection list"))?;
                if !links.iter().any(|v| v["@id"] == b) {
                    links.push(reference(&b));
                }
                let wire_id = format!("{id}.wire.{index}");
                let mut meta = Map::new();
                write_meta(&mut meta, &wire.meta);
                if !meta.is_empty() {
                    let mut wire_node = node(&wire_id, "ace:Connection");
                    wire_node.extend(meta);
                    wire_node.insert("ace:from".into(), reference(a));
                    wire_node.insert("ace:to".into(), reference(b));
                    self.add(&wire_id, wire_node)?;
                    annotated.push(reference(wire_id));
                }
            }
            if !annotated.is_empty() {
                record.insert("ace:connections".into(), Json::Array(annotated));
            }
        }
        self.add(id, record)
    }
    fn instance(&mut self, id: &str, instance: &Instance) -> Result<(), Diagnostic> {
        let class = self
            .document
            .library
            .classes
            .get(&instance.class)
            .ok_or_else(|| error(id, format!("unknown class {}", instance.class)))?;
        let mut record = node(id, &class.id.0);
        write_meta(&mut record, &instance.meta);

        if instance
            .meta
            .label
            .as_deref()
            .is_some_and(|label| label != instance.name)
        {
            record.insert(
                "ace:instanceName".into(),
                Json::String(instance.name.clone()),
            );
        }
        let mut ports = class
            .inputs
            .iter()
            .chain(&class.outputs)
            .cloned()
            .collect::<Vec<_>>();
        for p in &instance.connectors {
            if let Some(i) = ports.iter().position(|port| port.name == p.name) {
                ports[i] = p.clone();
            } else {
                ports.push(p.clone());
            }
        }
        let mut declarations = class.parameters.clone();
        for p in &instance.parameters {
            if let Some(i) = declarations.iter().position(|decl| decl.name == p.name) {
                declarations[i] = p.clone();
            } else {
                declarations.push(p.clone());
            }
        }
        let mut local = Vec::new();
        for (key, direction) in [
            ("S231:hasInput", Direction::Input),
            ("S231:hasOutput", Direction::Output),
        ] {
            let mut references = Vec::new();
            for port in ports.iter().filter(|p| p.direction == direction) {
                let port_id = format!("{id}.{}", encode(&port.name));
                self.port(&port_id, port)?;
                if instance.connectors.iter().any(|p| p.name == port.name) {
                    self.nodes
                        .get_mut(&port_id)
                        .unwrap()
                        .insert("ace:connectorOverride".into(), Json::Bool(true));
                }
                let r = reference(port_id);
                references.push(r.clone());
                local.push(r);
            }
            if !references.is_empty() {
                record.insert(key.into(), Json::Array(references));
            }
        }
        let mut parameters = Vec::new();
        // Only bindings become instance values. Class defaults remain on the class.
        for assignment in &instance.assignments {
            let declaration = declarations
                .iter()
                .find(|p| p.name == assignment.name)
                .ok_or_else(|| error(id, format!("unknown parameter {}", assignment.name)))?;
            let p_id = format!("{id}.{}", encode(&assignment.name));
            let mut local_declaration = declaration.clone();
            if !instance
                .parameters
                .iter()
                .any(|p| p.name == assignment.name)
            {
                local_declaration.meta = Meta::default();
                local_declaration.units = Units::default();
                local_declaration.min = None;
                local_declaration.max = None;
                local_declaration.dimensions.clear();
                local_declaration.default = None;
            }
            self.parameter(&p_id, &local_declaration, Some(assignment), false)?;
            let r = reference(p_id);
            parameters.push(r.clone());
            local.push(r);
        }
        for declaration in &instance.parameters {
            let inherited = class.parameters.iter().find(|p| p.name == declaration.name);
            let bound = instance
                .assignments
                .iter()
                .any(|p| p.name == declaration.name);
            let p_id = format!("{id}.{}", encode(&declaration.name));
            if !bound && inherited != Some(declaration) {
                self.parameter(&p_id, declaration, None, false)?;
                self.nodes.get_mut(&p_id).unwrap().remove("S231:value");
                let r = reference(&p_id);
                parameters.push(r.clone());
                local.push(r);
            }
            if inherited.map(|p| &p.default) != Some(&declaration.default) {
                if let Some(default) = &declaration.default {
                    if let Some(node) = self.nodes.get_mut(&p_id) {
                        node.insert(
                            "ace:declarationDefault".into(),
                            scalar_json(default, &declaration.data_type),
                        );
                    }
                } else if inherited.is_some() {
                    if let Some(node) = self.nodes.get_mut(&p_id) {
                        node.insert("ace:clearsDefault".into(), Json::Bool(true));
                    }
                }
            }
        }
        if !parameters.is_empty() {
            record.insert("S231:hasParameter".into(), Json::Array(parameters));
        }
        if !local.is_empty() {
            record.insert("S231:hasInstance".into(), Json::Array(local));
        }
        self.add(id, record)
    }
}
