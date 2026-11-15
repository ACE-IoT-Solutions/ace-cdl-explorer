use crate::*;
use std::collections::{HashMap, HashSet};

type Code = DiagnosticCode;

fn diagnostic(out: &mut Vec<Diagnostic>, path: &str, code: Code, message: impl Into<String>) {
    out.push(Diagnostic::new(path, code, message));
}

fn resolve<'a>(
    class: &'a BlockClass,
    library: &'a Library,
    end: &Endpoint,
) -> Option<(&'a Connector, bool)> {
    if let Some(owner) = &end.instance {
        let child = class
            .body
            .as_ref()?
            .instances
            .iter()
            .find(|child| &child.name == owner)?;
        let declaration = library.classes.get(&child.class)?;
        let port = child
            .connectors
            .iter()
            .find(|port| port.name == end.connector)
            .or_else(|| declaration.connector(&end.connector))?;
        Some((port, port.direction == Direction::Output))
    } else {
        let port = class.connector(&end.connector)?;
        Some((port, port.direction == Direction::Input))
    }
}

fn oriented<'a>(
    class: &'a BlockClass,
    library: &'a Library,
    wire: &'a Connection,
) -> Option<(&'a Endpoint, &'a Endpoint)> {
    match (
        resolve(class, library, &wire.a)?,
        resolve(class, library, &wire.b)?,
    ) {
        ((_, true), (_, false)) => Some((&wire.a, &wire.b)),
        ((_, false), (_, true)) => Some((&wire.b, &wire.a)),
        _ => None,
    }
}

/// Stable topological child order for a composite class.
///
/// A loop-breaking class contributes no input dependencies. Unknown classes and
/// malformed wires produce diagnostics rather than being silently omitted.
pub fn evaluation_order(class: &BlockClass, library: &Library) -> Result<Vec<usize>, Diagnostic> {
    let Some(body) = &class.body else {
        return Ok(Vec::new());
    };
    let mut children = HashMap::new();
    for (index, child) in body.instances.iter().enumerate() {
        if children.insert(child.name.as_str(), index).is_some() {
            return Err(Diagnostic::new(
                &class.id.0,
                Code::DuplicateName,
                format!("duplicate child {}", child.name),
            ));
        }
        if !library.classes.contains_key(&child.class) {
            return Err(Diagnostic::new(
                &class.id.0,
                Code::UnknownClass,
                format!("unknown class {}", child.class),
            ));
        }
    }
    let mut dependencies = vec![HashSet::new(); body.instances.len()];
    for wire in &body.connections {
        let Some((source, target)) = oriented(class, library, wire) else {
            return Err(Diagnostic::new(
                &class.id.0,
                Code::MissingEndpoint,
                format!("cannot orient connection {} -- {}", wire.a, wire.b),
            ));
        };
        if let (Some(source), Some(target)) = (&source.instance, &target.instance) {
            let target_index = children[target.as_str()];
            if !library.classes[&body.instances[target_index].class].breaks_algebraic_loop {
                dependencies[target_index].insert(children[source.as_str()]);
            }
        }
    }
    let mut order = Vec::new();
    let mut done = HashSet::new();
    while order.len() != body.instances.len() {
        let next = dependencies
            .iter()
            .enumerate()
            .find(|(i, deps)| !done.contains(i) && deps.is_subset(&done));
        let Some((index, _)) = next else {
            return Err(Diagnostic::new(
                &class.id.0,
                Code::AlgebraicCycle,
                "connection cycle",
            ));
        };
        done.insert(index);
        order.push(index);
    }
    Ok(order)
}

fn value_matches(
    value: &Value,
    kind: &DataType,
    dimensions: &[Dimension],
    library: &Library,
) -> bool {
    if matches!(value, Value::Expr(_)) {
        return true;
    }
    if let Some((dimension, rest)) = dimensions.split_first() {
        let Value::Array(values) = value else {
            return false;
        };
        return !matches!(dimension, Dimension::Size(size) if *size != values.len() as u64)
            && values.iter().all(|v| value_matches(v, kind, rest, library));
    }
    match (value, kind) {
        (Value::Real(value), DataType::Real) => value.is_finite(),
        (Value::Integer(_), DataType::Real | DataType::Integer) => true,
        (Value::Boolean(_), DataType::Boolean) | (Value::String(_), DataType::String) => true,
        (Value::Enum(member), DataType::Enumeration(id)) => library
            .enumerations
            .get(id)
            .is_some_and(|enumeration| enumeration.members.contains(member)),
        _ => false,
    }
}

fn check_type(kind: &DataType, path: &str, library: &Library, out: &mut Vec<Diagnostic>) {
    if let DataType::Enumeration(id) = kind {
        if !library.enumerations.contains_key(id) {
            diagnostic(
                out,
                path,
                Code::UnknownClass,
                format!("unknown enumeration {id}"),
            );
        }
    }
}

fn check_class(class: &BlockClass, path: &str, library: &Library, out: &mut Vec<Diagnostic>) {
    let valid_iri = class.id.0.split_once(':').is_some_and(|(scheme, _)| {
        scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    }) && !class
        .id
        .0
        .chars()
        .any(|c| c.is_whitespace() || c.is_control());
    if !valid_iri {
        diagnostic(
            out,
            path,
            Code::InvalidClass,
            "class identifier must be an expanded IRI",
        );
    }
    if (class.kind == BlockKind::Composite) != class.body.is_some() {
        diagnostic(
            out,
            path,
            Code::InvalidClass,
            "only composite classes must have a body",
        );
    }
    let mut names = HashSet::new();
    for (ports, direction) in [
        (&class.inputs, Direction::Input),
        (&class.outputs, Direction::Output),
    ] {
        for port in ports {
            if port.name.trim().is_empty() || port.name.contains('.') {
                diagnostic(
                    out,
                    path,
                    Code::InvalidName,
                    format!("invalid connector name {}", port.name),
                );
            }
            if !names.insert(&port.name) {
                diagnostic(
                    out,
                    path,
                    Code::DuplicateName,
                    format!("duplicate connector {}", port.name),
                );
            }
            if port.direction != direction {
                diagnostic(
                    out,
                    path,
                    Code::InvalidDirection,
                    format!("wrong declared direction for {}", port.name),
                );
            }
            if port.dimensions.contains(&Dimension::Size(0)) {
                diagnostic(
                    out,
                    path,
                    Code::InvalidValue,
                    format!("zero array dimension for {}", port.name),
                );
            }
            check_type(&port.data_type, path, library, out);
        }
    }
    for parameter in class.parameters.iter().chain(&class.constants) {
        if !names.insert(&parameter.name) {
            diagnostic(
                out,
                path,
                Code::DuplicateName,
                format!("duplicate declaration {}", parameter.name),
            );
        }
        check_type(&parameter.data_type, path, library, out);
        if let Some(value) = &parameter.default {
            if !value_matches(value, &parameter.data_type, &parameter.dimensions, library) {
                diagnostic(
                    out,
                    path,
                    Code::InvalidValue,
                    format!("invalid default for {}", parameter.name),
                );
            }
        }
    }
    let Some(body) = &class.body else {
        return;
    };
    names.clear();
    for child in &body.instances {
        if !library.classes.contains_key(&child.class) {
            diagnostic(
                out,
                path,
                Code::UnknownClass,
                format!("unknown class {}", child.class),
            );
        }
        if !names.insert(&child.name) {
            diagnostic(
                out,
                path,
                Code::DuplicateName,
                format!("duplicate child {}", child.name),
            );
        }
    }
    let mut driven = HashSet::new();
    for wire in &body.connections {
        let a = resolve(class, library, &wire.a);
        let b = resolve(class, library, &wire.b);
        for (end, port) in [(&wire.a, a), (&wire.b, b)] {
            if port.is_none() {
                diagnostic(
                    out,
                    path,
                    Code::MissingEndpoint,
                    format!("missing endpoint {end}"),
                );
            }
        }
        let (Some((a, ad)), Some((b, bd))) = (a, b) else {
            continue;
        };
        if ad == bd {
            diagnostic(
                out,
                path,
                Code::InvalidDirection,
                format!("connection has two {}s", if ad { "source" } else { "sink" }),
            );
            continue;
        }
        if a.data_type != b.data_type || a.dimensions != b.dimensions {
            diagnostic(
                out,
                path,
                Code::TypeMismatch,
                format!("type mismatch {} -- {}", wire.a, wire.b),
            );
        }
        let target = if ad { &wire.b } else { &wire.a };
        if !driven.insert(target.clone()) {
            diagnostic(
                out,
                path,
                Code::MultipleDrivers,
                format!("multiple drivers for {target}"),
            );
        }
    }
    for child in &body.instances {
        if let Some(declaration) = library.classes.get(&child.class) {
            let ports = declaration
                .inputs
                .iter()
                .map(|port| {
                    child
                        .connectors
                        .iter()
                        .find(|p| p.name == port.name)
                        .unwrap_or(port)
                })
                .chain(child.connectors.iter().filter(|port| {
                    port.direction == Direction::Input
                        && declaration.connector(&port.name).is_none()
                }));
            for port in ports {
                let target = Endpoint::child(&child.name, &port.name);
                if port.required && !driven.contains(&target) {
                    diagnostic(
                        out,
                        path,
                        Code::UndrivenInput,
                        format!("missing input {target}"),
                    );
                }
            }
        }
    }
    for port in &class.outputs {
        if port.required && !driven.contains(&Endpoint::boundary(&port.name)) {
            diagnostic(
                out,
                path,
                Code::UndrivenInput,
                format!("undriven output {}", port.name),
            );
        }
    }
    if let Err(error) = evaluation_order(class, library) {
        if error.code == Code::AlgebraicCycle {
            diagnostic(out, path, error.code, error.message);
        }
    }
}

/// Validate class structure, reachable instances, assignments and wiring.
///
/// Expressions are preserved without evaluation, and implementation-specific
/// parameter rules belong to the block registry.
pub fn validate(document: &Document) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let (instances, traversal_errors) = document.walk();
    out.extend(traversal_errors);
    let mut checked = HashSet::new();
    for flat in &instances {
        let path = flat.path.to_string();
        if flat.instance.name.trim().is_empty() || flat.instance.name.contains('.') {
            diagnostic(
                &mut out,
                &path,
                Code::InvalidName,
                "empty or invalid instance name",
            );
        }
        if checked.insert(&flat.class.id) {
            check_class(flat.class, &path, &document.library, &mut out);
        }
        let mut connector_names = HashSet::new();
        for port in &flat.instance.connectors {
            if !connector_names.insert(&port.name) {
                diagnostic(
                    &mut out,
                    &path,
                    Code::DuplicateName,
                    format!("duplicate instance connector {}", port.name),
                );
            }
            if let Some(original) = flat.class.connector(&port.name) {
                if original.direction != port.direction {
                    diagnostic(
                        &mut out,
                        &path,
                        Code::InvalidDirection,
                        format!("connector {} overrides its direction", port.name),
                    );
                }
            }
            check_type(&port.data_type, &path, &document.library, &mut out);
        }
        let mut bindings = HashSet::new();
        for assignment in &flat.instance.assignments {
            if !bindings.insert(&assignment.name) {
                diagnostic(
                    &mut out,
                    &path,
                    Code::DuplicateName,
                    format!("duplicate binding {}", assignment.name),
                );
            }
            match flat
                .instance
                .parameters
                .iter()
                .find(|p| p.name == assignment.name)
                .or_else(|| {
                    flat.class
                        .parameters
                        .iter()
                        .find(|p| p.name == assignment.name)
                }) {
                None => diagnostic(
                    &mut out,
                    &path,
                    Code::UnknownParameter,
                    format!("unknown parameter {}", assignment.name),
                ),
                Some(parameter)
                    if !value_matches(
                        &assignment.value,
                        &parameter.data_type,
                        &parameter.dimensions,
                        &document.library,
                    ) =>
                {
                    diagnostic(
                        &mut out,
                        &path,
                        Code::InvalidValue,
                        format!("invalid parameter value {}", assignment.name),
                    );
                }
                _ => {}
            }
        }
    }
    // Unused classes are still part of a document and must not hide malformed wiring.
    for (id, class) in &document.library.classes {
        if id != &class.id {
            diagnostic(
                &mut out,
                &id.0,
                Code::InvalidClass,
                "class key and IRI disagree",
            );
        }
        if !checked.contains(id) {
            check_class(class, &id.0, &document.library, &mut out);
        }
    }
    out
}
