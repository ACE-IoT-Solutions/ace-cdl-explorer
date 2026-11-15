use crate::format::*;
use cdl_model::*;
use serde_json::{Map, Value as Json};
use std::collections::{BTreeMap, HashSet};

const INTERFACE: &[&str] = &[
    "ace:connections",
    "ace:instanceName",
    "S231:hasInput",
    "S231:hasOutput",
    "S231:hasParameter",
    "S231:hasConstant",
    "S231:hasInstance",
    "S231:containsBlock",
];
const PORT: &[&str] = &[
    "ace:connectorOverride",
    "S231:isOfDataType",
    "qudt:hasUnit",
    "qudt:hasQuantityKind",
    "S231:hasDisplayUnit",
    "ace:required",
    "S231:isArray",
    "S231:numberDimensions",
    "S231:sizeOfDimensions",
];
const PARAM: &[&str] = &[
    "qudt:hasUnit",
    "qudt:hasQuantityKind",
    "S231:hasDisplayUnit",
    "S231:isArray",
    "S231:numberDimensions",
    "S231:sizeOfDimensions",
    "S231:value",
    "ace:declarationDefault",
    "ace:clearsDefault",
    "S231:min",
    "S231:max",
    "S231:isOfDataType",
];
fn kind(node: &Map<String, Json>) -> &str {
    node.get("@type").and_then(Json::as_str).unwrap_or("")
}
fn expanded_class(id: &str) -> String {
    if let Some(local) = id.strip_prefix("ace:") {
        return format!("{ACE}{local}");
    }
    class_id(
        &id.strip_prefix("S231:")
            .map(|s| format!("{S}{s}"))
            .unwrap_or_else(|| id.into()),
    )
}
fn is_class(node: &Map<String, Json>) -> bool {
    matches!(
        kind(node),
        "S231:Block" | "S231:ElementaryBlock" | "S231:CompositeBlock" | "S231:ExtensionBlock"
    )
}

/// Import a compacted CXF JSON-LD document using caller-supplied class declarations.
pub fn from_json(input: &str, library: &Library) -> Result<Document, Vec<Diagnostic>> {
    let value = serde_json::from_str(input).map_err(|e| vec![error("", e.to_string())])?;
    from_value(&value, library)
}
/// Import a compacted CXF graph, selecting its unique unreferenced block root.
pub fn from_value(input: &Json, library: &Library) -> Result<Document, Vec<Diagnostic>> {
    read(input, library, None).map_err(|e| vec![e])
}
/// Import a graph with an explicitly selected root node IRI.
pub fn from_value_with_root(
    input: &Json,
    library: &Library,
    root: &str,
) -> Result<Document, Vec<Diagnostic>> {
    read(input, library, Some(root)).map_err(|e| vec![e])
}
fn read(input: &Json, library: &Library, selected: Option<&str>) -> Result<Document, Diagnostic> {
    let index = Index::parse(input)?;
    let mut reader = Reader {
        index,
        library: library.clone(),
        active: HashSet::new(),
    };
    let referenced: HashSet<_> = reader
        .index
        .nodes
        .values()
        .flat_map(|n| {
            refs(n.get("S231:containsBlock"))
                .into_iter()
                .chain(std::iter::once(expanded_class(kind(n))))
        })
        .collect();
    let candidates: Vec<_> = reader
        .index
        .order
        .iter()
        .filter(|id| {
            (is_class(&reader.index.nodes[*id])
                || reader
                    .library
                    .classes
                    .contains_key(expanded_class(kind(&reader.index.nodes[*id])).as_str())
                || reader
                    .index
                    .nodes
                    .contains_key(&expanded_class(kind(&reader.index.nodes[*id]))))
                && !referenced.contains(*id)
        })
        .cloned()
        .collect();
    let declared_root = refs(reader.index.meta.get("ace:rootBlock"))
        .into_iter()
        .next();
    let root = if let Some(root) = selected.or(declared_root.as_deref()) {
        root.to_owned()
    } else if candidates.len() == 1 {
        candidates[0].clone()
    } else {
        return Err(error(
            "@graph",
            format!(
                "expected one root block; found {} (select a root explicitly)",
                candidates.len()
            ),
        ));
    };
    let node = reader.node(&root)?.clone();
    let class = expanded_class(&root);
    let mut instance = if is_class(&node) {
        if !reader.library.classes.contains_key(class.as_str()) {
            reader.load_class(&root, &class, None)?;
        }
        reader.instance(&root, Some(&class))?
    } else {
        reader.instance(&root, None)?
    };
    instance.name = text(node.get("ace:instanceName"))
        .or_else(|| text(node.get("S231:label")))
        .unwrap_or_else(|| name(&root));
    for id in reader.index.order.clone() {
        let n = &reader.index.nodes[&id];
        if is_class(n)
            && !reader
                .library
                .classes
                .contains_key(expanded_class(&id).as_str())
        {
            reader.load_class(&id, &expanded_class(&id), None)?;
        } else if kind(n) == "S231:EnumerationType" {
            let members = n
                .get("ace:enumerationMembers")
                .and_then(Json::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Json::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            let e = Enumeration {
                id: expanded_class(&id).into(),
                members,
                meta: metadata(n, &["ace:enumerationMembers"]),
            };
            reader.library.enumerations.insert(e.id.clone(), e);
        }
    }
    reader.library.meta = metadata(&reader.index.meta, &["ace:rootBlock"]);
    let document = Document {
        library: reader.library,
        root: instance,
    };
    // Imported expressions and unimplemented elementary classes remain inspectable.
    // Structural diagnostics stay available through cdl_model::validate; no behavior
    // or default expression evaluation is performed by this format reader.
    Ok(document)
}
struct Reader {
    index: Index,
    library: Library,
    active: HashSet<String>,
}
impl Reader {
    fn node(&self, id: &str) -> Result<&Map<String, Json>, Diagnostic> {
        self.index
            .nodes
            .get(id)
            .ok_or_else(|| error(id, "missing referenced node"))
    }
    fn connector(&self, id: &str, direction: Direction) -> Result<Connector, Diagnostic> {
        let node = self.node(id)?;
        if kind(node).ends_with("Input") && direction != Direction::Input
            || kind(node).ends_with("Output") && direction != Direction::Output
        {
            return Err(Diagnostic::new(
                id,
                DiagnosticCode::InvalidDirection,
                "connector direction disagrees with its reference role",
            ));
        }
        let mut port = Connector::new(
            name(id),
            direction,
            refs(node.get("S231:isOfDataType"))
                .first()
                .map(|s| dtype(s))
                .unwrap_or_else(|| {
                    if kind(node).is_empty() {
                        DataType::Real
                    } else {
                        dtype(kind(node).strip_prefix("S231:").unwrap_or(kind(node)))
                    }
                }),
        );
        port.required = node
            .get("ace:required")
            .and_then(Json::as_bool)
            .unwrap_or(true);
        port.meta = metadata(node, PORT);
        port.units = units(node);
        port.dimensions = dimensions(node);
        Ok(port)
    }
    fn parameter(&self, id: &str) -> Result<ParameterDecl, Diagnostic> {
        let node = self.node(id)?;
        let data_type = refs(node.get("S231:isOfDataType"))
            .first()
            .map(|s| dtype(s))
            .unwrap_or_else(|| {
                let enum_id = node
                    .get("S231:value")
                    .and_then(|v| literal(v).as_str())
                    .and_then(|v| v.rsplit_once('.'))
                    .map(|(id, _)| {
                        if id.starts_with("Buildings.") {
                            format!("{S}{id}")
                        } else {
                            expanded_class(id)
                        }
                    });
                if let Some(id) = enum_id.filter(|id| {
                    self.library.enumerations.contains_key(id.as_str())
                        || self
                            .index
                            .nodes
                            .get(id)
                            .is_some_and(|n| kind(n) == "S231:EnumerationType")
                }) {
                    DataType::Enumeration(id.into())
                } else if node
                    .get("S231:value")
                    .is_some_and(|v| literal(v).is_boolean())
                {
                    DataType::Boolean
                } else if node
                    .get("S231:value")
                    .and_then(|v| v.get("@type"))
                    .and_then(Json::as_str)
                    .is_some_and(|t| t.ends_with("#string"))
                {
                    DataType::String
                } else {
                    DataType::Real
                }
            });
        Ok(ParameterDecl {
            name: name(id),
            default: node.get("S231:value").map(|v| scalar(v, &data_type)),
            min: node.get("S231:min").map(|v| scalar(v, &data_type)),
            max: node.get("S231:max").map(|v| scalar(v, &data_type)),
            data_type,
            units: units(node),
            dimensions: dimensions(node),
            meta: metadata(node, PARAM),
        })
    }
    fn interface(
        &self,
        node: &Map<String, Json>,
        class: &mut BlockClass,
    ) -> Result<(), Diagnostic> {
        for (field, direction) in [
            ("S231:hasInput", Direction::Input),
            ("S231:hasOutput", Direction::Output),
        ] {
            if node.contains_key(field) {
                let ports = refs(node.get(field))
                    .iter()
                    .map(|id| self.connector(id, direction))
                    .collect::<Result<_, _>>()?;
                if direction == Direction::Input {
                    class.inputs = ports;
                } else {
                    class.outputs = ports;
                }
            }
        }
        for (field, constant) in [("S231:hasParameter", false), ("S231:hasConstant", true)] {
            if node.contains_key(field) {
                let params = refs(node.get(field))
                    .iter()
                    .map(|id| self.parameter(id))
                    .collect::<Result<_, _>>()?;
                if constant {
                    class.constants = params;
                } else {
                    class.parameters = params;
                }
            }
        }
        Ok(())
    }
    fn load_class(
        &mut self,
        source: &str,
        id: &str,
        base: Option<BlockClass>,
    ) -> Result<(), Diagnostic> {
        if !self.active.insert(source.into()) {
            return Err(error(source, "recursive block definition"));
        }
        let node = self.node(source)?.clone();
        let children = refs(node.get("S231:containsBlock"));
        let boundary_wires = self.index.nodes.iter().any(|(key, node)| {
            key.strip_prefix(&format!("{source}."))
                .is_some_and(|local| !local.contains('.'))
                && (node.contains_key("S231:isConnectedTo")
                    || node.contains_key("S231:connectedTo"))
        });
        let interface_only = kind(&node) == "S231:CompositeBlock"
            && !node.contains_key("S231:containsBlock")
            && !boundary_wires;
        let class_kind = if !children.is_empty()
            || boundary_wires
            || kind(&node) == "S231:CompositeBlock" && !interface_only
        {
            BlockKind::Composite
        } else if kind(&node) == "S231:ElementaryBlock" && !id.starts_with(ACE) {
            BlockKind::Elementary
        } else {
            BlockKind::Extension
        };
        let mut class = base.unwrap_or_else(|| BlockClass::new(id, class_kind));
        class.id = id.into();
        class.kind = class_kind;
        class.meta = metadata(&node, INTERFACE);
        if interface_only {
            class
                .meta
                .annotations
                .insert("ace:interfaceOnly".into(), Annotation::Boolean(true));
        } else {
            class.meta.annotations.shift_remove("ace:interfaceOnly");
        }
        self.interface(&node, &mut class)?;
        class.breaks_algebraic_loop = matches!(name(id).as_str(), "UnitDelay" | "Pre");
        class.body = if class_kind == BlockKind::Composite {
            Some(CompositeBody::default())
        } else {
            None
        };
        // Install the interface before visiting children. Shared templates are legal;
        // recursive class references are diagnosed structurally after import.
        self.library.classes.insert(class.id.clone(), class.clone());
        if let Some(body) = &mut class.body {
            for child in &children {
                body.instances.push(self.instance(child, None)?);
            }
        }
        if class.kind == BlockKind::Composite {
            let wires = self.connections(source, &class, &children)?;
            if let Some(body) = &mut class.body {
                body.connections = wires;
            }
        }
        self.library.classes.insert(class.id.clone(), class);
        self.active.remove(source);
        Ok(())
    }
    fn instance(&mut self, id: &str, forced: Option<&str>) -> Result<Instance, Diagnostic> {
        let node = self.node(id)?.clone();
        let mut class = forced
            .map(str::to_owned)
            .unwrap_or_else(|| expanded_class(kind(&node)));
        if !self.library.classes.contains_key(class.as_str()) {
            let source = if self.index.nodes.contains_key(&class) {
                class.clone()
            } else if self.index.nodes.contains_key(kind(&node)) {
                kind(&node).to_owned()
            } else {
                return Err(Diagnostic::new(
                    id,
                    DiagnosticCode::UnknownClass,
                    format!("unknown class {class}"),
                ));
            };
            self.load_class(&source, &class, None)?;
        }
        if forced.is_none() && node.contains_key("S231:containsBlock") {
            let specialized = format!("{id}~class");
            let base = self.library.classes.get(class.as_str()).cloned();
            self.load_class(id, &specialized, base)?;
            class = specialized;
        }
        let definition = self.library.classes[class.as_str()].clone();
        let mut instance = Instance::new(name(id), class.as_str());
        instance.meta = metadata(&node, INTERFACE);
        instance.connectors = definition
            .inputs
            .iter()
            .chain(&definition.outputs)
            .cloned()
            .collect();
        instance.parameters = definition.parameters.clone();
        let mut local = refs(node.get("S231:hasInstance"));
        for field in ["S231:hasInput", "S231:hasOutput", "S231:hasParameter"] {
            for reference in refs(node.get(field)) {
                if !local.contains(&reference) {
                    local.push(reference);
                }
            }
        }
        // A class-referenced instance may have local connector/parameter records
        // without a hasInstance list (older modelica-json output).
        for declaration in definition
            .inputs
            .iter()
            .map(|p| p.name.as_str())
            .chain(definition.outputs.iter().map(|p| p.name.as_str()))
            .chain(definition.parameters.iter().map(|p| p.name.as_str()))
        {
            let candidate = format!("{id}.{declaration}");
            if self.index.nodes.contains_key(&candidate) && !local.contains(&candidate) {
                local.push(candidate);
            }
        }
        for local_id in local {
            let local_name = name(&local_id);
            let source = match self.index.nodes.get(&local_id) {
                Some(source) => source.clone(),
                None if instance.connectors.iter().any(|p| p.name == local_name)
                    || instance.parameters.iter().any(|p| p.name == local_name) =>
                {
                    continue
                }
                None => return Err(error(&local_id, "missing instance declaration")),
            };
            let port_index = instance
                .connectors
                .iter()
                .position(|p| p.name == local_name);
            let declared_input = refs(node.get("S231:hasInput")).contains(&local_id);
            let declared_output = refs(node.get("S231:hasOutput")).contains(&local_id);
            if port_index.is_some()
                || kind(&source).ends_with("Input")
                || kind(&source).ends_with("Output")
                || declared_input
                || declared_output
            {
                let mut port = if let Some(index) = port_index {
                    if source.get("ace:connectorOverride").and_then(Json::as_bool) == Some(true) {
                        self.connector(&local_id, instance.connectors[index].direction)?
                    } else {
                        instance.connectors[index].clone()
                    }
                } else {
                    self.connector(
                        &local_id,
                        if kind(&source).ends_with("Input") || declared_input {
                            Direction::Input
                        } else {
                            Direction::Output
                        },
                    )?
                };
                if kind(&source).ends_with("Input") && port.direction != Direction::Input
                    || kind(&source).ends_with("Output") && port.direction != Direction::Output
                {
                    return Err(Diagnostic::new(
                        &local_id,
                        DiagnosticCode::InvalidDirection,
                        "instance connector overrides its direction",
                    ));
                }
                if let Some(kind_id) = refs(source.get("S231:isOfDataType")).first() {
                    port.data_type = dtype(kind_id);
                } else if kind(&source).ends_with("Input") || kind(&source).ends_with("Output") {
                    port.data_type =
                        dtype(kind(&source).strip_prefix("S231:").unwrap_or(kind(&source)));
                }
                if let Some(required) = source.get("ace:required").and_then(Json::as_bool) {
                    port.required = required;
                }
                let local_meta = metadata(&source, PORT);
                if local_meta.label.is_some() {
                    port.meta.label = local_meta.label;
                }
                if local_meta.description.is_some() {
                    port.meta.description = local_meta.description;
                }
                port.meta.annotations.extend(local_meta.annotations);
                let u = units(&source);
                if u.unit.is_some() {
                    port.units.unit = u.unit;
                }
                if u.display_unit.is_some() {
                    port.units.display_unit = u.display_unit;
                }
                if u.quantity_kind.is_some() {
                    port.units.quantity_kind = u.quantity_kind;
                }
                if source.contains_key("S231:sizeOfDimensions") {
                    port.dimensions = dimensions(&source);
                }
                if let Some(index) = port_index {
                    instance.connectors[index] = port;
                } else {
                    instance.connectors.push(port);
                }
                continue;
            }
            let p = if let Some(index) = instance
                .parameters
                .iter()
                .position(|p| p.name == local_name)
            {
                index
            } else if kind(&source) == "S231:Parameter"
                || source.contains_key("S231:value")
                || refs(node.get("S231:hasParameter")).contains(&local_id)
            {
                let mut p = self.parameter(&local_id)?;
                p.default = None;
                if source.contains_key("S231:value") {
                    p.meta
                        .annotations
                        .retain(|k, _| k.starts_with("S231:") && k != "S231:isFinal");
                }
                instance.parameters.push(p);
                instance.parameters.len() - 1
            } else {
                continue;
            };
            let declaration = &mut instance.parameters[p];
            if let Some(kind_id) = refs(source.get("S231:isOfDataType")).first() {
                declaration.data_type = dtype(kind_id);
            }
            let m = metadata(&source, PARAM);
            if m.label.is_some() {
                declaration.meta.label = m.label.clone();
            }
            if m.description.is_some() {
                declaration.meta.description = m.description.clone();
            }
            let inherited = definition.parameters.iter().find(|p| p.name == local_name);
            let mut binding_meta = Meta::default();
            for (key, value) in &m.annotations {
                if key == "S231:isFinal"
                    || source.contains_key("S231:value")
                        && !key.starts_with("S231:")
                        && inherited.and_then(|p| p.meta.annotations.get(key)) != Some(value)
                {
                    binding_meta.annotations.insert(key.clone(), value.clone());
                } else {
                    declaration
                        .meta
                        .annotations
                        .insert(key.clone(), value.clone());
                }
            }
            let u = units(&source);
            if u.unit.is_some() {
                declaration.units.unit = u.unit;
            }
            if u.display_unit.is_some() {
                declaration.units.display_unit = u.display_unit;
            }
            if u.quantity_kind.is_some() {
                declaration.units.quantity_kind = u.quantity_kind;
            }
            if source.contains_key("S231:sizeOfDimensions") || source.contains_key("S231:isArray") {
                declaration.dimensions = dimensions(&source);
            }
            if let Some(value) = source.get("S231:min") {
                declaration.min = Some(scalar(value, &declaration.data_type));
            }
            if let Some(value) = source.get("S231:max") {
                declaration.max = Some(scalar(value, &declaration.data_type));
            }
            if let Some(value) = source.get("ace:declarationDefault") {
                declaration.default = Some(scalar(value, &declaration.data_type));
            }
            if source.get("ace:clearsDefault").and_then(Json::as_bool) == Some(true) {
                if source.contains_key("ace:declarationDefault") {
                    return Err(error(&local_id, "conflicting default override fields"));
                }
                declaration.default = None;
            }
            if let Some(v) = source.get("S231:value") {
                instance.assignments.push(Assignment {
                    name: local_name,
                    value: scalar(v, &declaration.data_type),
                    meta: binding_meta,
                });
            }
        }
        self.resolve_interface_variant(&mut instance, &definition, id)?;
        let definition = &self.library.classes[&instance.class];
        instance
            .connectors
            .retain(|p| definition.connector(&p.name) != Some(p));
        instance
            .parameters
            .retain(|p| definition.parameters.iter().find(|d| d.name == p.name) != Some(p));
        Ok(instance)
    }
    fn resolve_interface_variant(
        &mut self,
        instance: &mut Instance,
        definition: &BlockClass,
        source: &str,
    ) -> Result<(), Diagnostic> {
        let origin = match instance
            .meta
            .annotations
            .get("ace:blockType")
            .or_else(|| definition.meta.annotations.get("ace:blockType"))
        {
            Some(Annotation::String(v)) => Some(v.clone()),
            _ => None,
        };
        if definition.id.0.starts_with(ACE) {
            if let Some(origin) = &origin {
                let class = self.library.classes.get_mut(&instance.class).unwrap();
                if let Some(Annotation::String(existing)) =
                    class.meta.annotations.get("ace:blockType")
                {
                    if existing != origin {
                        return Err(error(
                            source,
                            "conflicting ace:blockType declarations on a shared class",
                        ));
                    }
                } else {
                    class
                        .meta
                        .annotations
                        .insert("ace:blockType".into(), Annotation::String(origin.clone()));
                }
            }
        }
        if definition.kind != BlockKind::Elementary
            || !definition
                .id
                .0
                .starts_with(&format!("{S}Buildings.Controls.OBC.CDL."))
        {
            return Ok(());
        }
        let changed_ports = instance.connectors.iter().any(|p| {
            definition.connector(&p.name).is_none_or(|base| {
                base.data_type != p.data_type
                    || base.direction != p.direction
                    || base.dimensions != p.dimensions
            })
        });
        let changed_parameters = instance.parameters.iter().any(|p| {
            definition
                .parameters
                .iter()
                .find(|base| base.name == p.name)
                .is_none_or(|base| base.data_type != p.data_type || base.dimensions != p.dimensions)
        });
        let changed_family = origin
            .as_ref()
            .is_some_and(|v| name(v) != name(&definition.id.0));
        if !(changed_ports || changed_parameters || changed_family) {
            return Ok(());
        }
        let mut variant = definition.clone();
        variant.id = format!(
            "{ACE}interface.{}",
            source
                .bytes()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        )
        .into();
        variant.kind = BlockKind::Extension;
        variant.meta.annotations.insert(
            "ace:blockType".into(),
            Annotation::String(origin.unwrap_or_else(|| {
                definition
                    .id
                    .0
                    .strip_prefix(S)
                    .unwrap_or(&definition.id.0)
                    .into()
            })),
        );
        variant.meta.annotations.insert(
            "ace:baseClass".into(),
            Annotation::Object(IndexMap::from_iter([(
                "@id".into(),
                Annotation::String(definition.id.0.clone()),
            )])),
        );
        for p in &instance.connectors {
            let ports = if p.direction == Direction::Input {
                &mut variant.inputs
            } else {
                &mut variant.outputs
            };
            if let Some(base) = ports.iter_mut().find(|base| base.name == p.name) {
                base.data_type = p.data_type.clone();
                base.dimensions = p.dimensions.clone();
            } else {
                let mut declaration = p.clone();
                declaration.meta = Meta::default();
                ports.push(declaration);
            }
        }
        for p in &instance.parameters {
            if let Some(base) = variant
                .parameters
                .iter_mut()
                .find(|base| base.name == p.name)
            {
                base.data_type = p.data_type.clone();
                base.dimensions = p.dimensions.clone();
            } else {
                let mut declaration = p.clone();
                declaration.meta = Meta::default();
                variant.parameters.push(declaration);
            }
        }
        if self.library.classes.contains_key(&variant.id) {
            return Err(error(
                source,
                "generated interface variant conflicts with an existing class",
            ));
        }
        instance.class = variant.id.clone();
        self.library.classes.insert(variant.id.clone(), variant);
        Ok(())
    }
    fn connections(
        &self,
        id: &str,
        class: &BlockClass,
        children: &[String],
    ) -> Result<Vec<Connection>, Diagnostic> {
        let mut endpoints: BTreeMap<String, (Endpoint, bool)> = BTreeMap::new();
        for port in class.inputs.iter().chain(&class.outputs) {
            endpoints.insert(
                format!("{id}.{}", port.name),
                (
                    Endpoint::boundary(&port.name),
                    port.direction == Direction::Input,
                ),
            );
        }
        for (source, child) in children.iter().zip(&class.body.as_ref().unwrap().instances) {
            let definition = &self.library.classes[&child.class];
            for port in child
                .connectors
                .iter()
                .chain(definition.inputs.iter().chain(&definition.outputs))
            {
                endpoints
                    .entry(format!("{source}.{}", port.name))
                    .or_insert((
                        Endpoint::child(&child.name, &port.name),
                        port.direction == Direction::Output,
                    ));
            }
        }
        let mut added = HashSet::new();
        let mut wires = Vec::new();
        for (left, (a, a_driver)) in &endpoints {
            let Some(node) = self.index.nodes.get(left) else {
                continue;
            };
            for right in refs(node.get("S231:isConnectedTo"))
                .into_iter()
                .chain(refs(node.get("S231:connectedTo")))
            {
                let Some((b, b_driver)) = endpoints.get(&right) else {
                    if right
                        .strip_prefix(&format!("{id}."))
                        .is_some_and(|s| s.split('.').count() <= 2)
                    {
                        return Err(Diagnostic::new(
                            left,
                            DiagnosticCode::MissingEndpoint,
                            format!("missing wire endpoint {right}"),
                        ));
                    }
                    continue;
                };
                if a_driver == b_driver {
                    return Err(Diagnostic::new(
                        left,
                        DiagnosticCode::InvalidDirection,
                        format!("ambiguous wire direction {left} -- {right}"),
                    ));
                }
                let key = if left < &right {
                    (left.clone(), right.clone())
                } else {
                    (right.clone(), left.clone())
                };
                if added.insert(key) {
                    let (a, b) = if *a_driver {
                        (a.clone(), b.clone())
                    } else {
                        (b.clone(), a.clone())
                    };
                    wires.push(Connection {
                        a,
                        b,
                        meta: Meta::default(),
                    });
                }
            }
        }
        let owner = self.node(id)?;
        for reference in refs(owner.get("ace:connections")) {
            let record = self.node(&reference)?;
            let a_id = refs(record.get("ace:from"))
                .into_iter()
                .next()
                .ok_or_else(|| error(&reference, "connection metadata requires ace:from"))?;
            let b_id = refs(record.get("ace:to"))
                .into_iter()
                .next()
                .ok_or_else(|| error(&reference, "connection metadata requires ace:to"))?;
            let (a, _) = endpoints
                .get(&a_id)
                .ok_or_else(|| error(&reference, format!("missing endpoint {a_id}")))?;
            let (b, _) = endpoints
                .get(&b_id)
                .ok_or_else(|| error(&reference, format!("missing endpoint {b_id}")))?;
            let wire = wires
                .iter_mut()
                .find(|w| (&w.a == a && &w.b == b) || (&w.a == b && &w.b == a))
                .ok_or_else(|| error(&reference, "metadata references an absent wire"))?;
            wire.meta = metadata(record, &["ace:from", "ace:to"]);
        }
        Ok(wires)
    }
}
