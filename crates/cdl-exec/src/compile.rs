use crate::instruction::Instruction;
use crate::{Data, Error, PortHandle, Program};
use cdl_blocks::{BlockImpl, ConfiguredInstance, Registry, SlotAlloc, Step};
use cdl_model::{
    BlockClass, DataType, Diagnostic, DiagnosticCode, Direction, Document, Endpoint, Instance,
};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

fn key(path: &str, end: &Endpoint) -> String {
    match &end.instance {
        Some(child) => format!("{path}.{child}.{}", end.connector),
        None => format!("{path}.{}", end.connector),
    }
}
fn root(parents: &mut [usize], index: usize) -> usize {
    if parents[index] != index {
        parents[index] = root(parents, parents[index]);
    }
    parents[index]
}
impl Program {
    /// Validate and compile a document using the authoritative registry.
    /// Unsupported classes fail here; there is no fallback execution engine.
    pub fn compile(document: &Document, registry: &Registry) -> Result<Self, Error> {
        let errors = cdl_model::validate(document);
        if !errors.is_empty() {
            return Err(Error::Diagnostics(errors));
        }
        let flat = document.instances().map_err(Error::Diagnostics)?;
        let mut ports: HashMap<String, (usize, DataType)> = HashMap::new();
        for item in &flat {
            for port in item.class.inputs.iter().chain(&item.class.outputs) {
                let port = item
                    .instance
                    .connectors
                    .iter()
                    .find(|p| p.name == port.name)
                    .unwrap_or(port);
                if !port.dimensions.is_empty()
                    || !matches!(
                        port.data_type,
                        DataType::Real | DataType::Integer | DataType::Boolean
                    )
                {
                    return Err(Error::Diagnostics(vec![Diagnostic::new(
                        item.path.to_string(),
                        DiagnosticCode::TypeMismatch,
                        "executor requires scalar Real, Integer or Boolean connectors",
                    )]));
                }
                let index = ports.len();
                ports.insert(
                    format!("{}.{}", item.path, port.name),
                    (index, port.data_type.clone()),
                );
            }
        }
        let mut parents: Vec<_> = (0..ports.len()).collect();
        for item in &flat {
            if let Some(body) = &item.class.body {
                let path = item.path.to_string();
                for wire in &body.connections {
                    let a = ports[&key(&path, &wire.a)].0;
                    let b = ports[&key(&path, &wire.b)].0;
                    let a = root(&mut parents, a);
                    let b = root(&mut parents, b);
                    parents[b] = a;
                }
            }
        }
        let mut slots = HashMap::new();
        for (index, _) in ports.values_mut() {
            let representative = root(&mut parents, *index);
            let next = slots.len();
            *index = *slots.entry(representative).or_insert(next);
        }
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let handle = |path: &str| {
            let (slot, data_type) = &ports[path];
            PortHandle {
                program: id,
                slot: *slot,
                data_type: data_type.clone(),
                input: None,
            }
        };
        let mut data = Data {
            id,
            #[cfg(feature = "serde")]
            constant_slots: Vec::new(),
            #[cfg(feature = "serde")]
            fingerprint: crate::snapshot::fingerprint(document)?,
            #[cfg(feature = "serde")]
            signal_paths: ports
                .iter()
                .map(|(path, (slot, _))| (path.clone(), *slot))
                .collect(),
            slots: vec![0.0; slots.len()],
            kernels: vec![],
            paths: vec![],
            stateful: vec![],
            breakers: vec![],
            previous: vec![],
            periods: vec![],
            inputs: HashMap::new(),
            outputs: HashMap::new(),
            traces: HashMap::new(),
        };
        for port in &flat[0].class.inputs {
            let mut h = handle(&format!("{}.{}", document.root.name, port.name));
            h.input = Some(data.inputs.len());
            data.inputs.insert(port.name.clone(), h);
        }
        for port in &flat[0].class.outputs {
            data.outputs.insert(
                port.name.clone(),
                handle(&format!("{}.{}", document.root.name, port.name)),
            );
        }
        for path in ports.keys() {
            data.traces.insert(path.clone(), handle(path));
        }
        for item in &flat {
            if let Some(body) = &item.class.body {
                let path = item.path.to_string();
                for port in &item.class.inputs {
                    data.traces.insert(
                        format!("{path}|{}", port.name),
                        handle(&format!("{path}.{}", port.name)),
                    );
                }
                for port in &item.class.outputs {
                    data.traces.insert(
                        format!("{path}|output.{}", port.name),
                        handle(&format!("{path}.{}", port.name)),
                    );
                }
                for child in &body.instances {
                    for port in &document.library.classes[&child.class].outputs {
                        data.traces.insert(
                            format!("{path}|{}.{}", child.name, port.name),
                            handle(&format!("{path}.{}.{}", child.name, port.name)),
                        );
                    }
                }
                for (index, wire) in body.connections.iter().enumerate() {
                    data.traces
                        .insert(format!("{path}|edge.{index}"), handle(&key(&path, &wire.a)));
                }
            }
        }
        // Expand composites in the model's stable topological order. Cycle
        // breakers have allocated slots before any leaf is built.
        let mut pending: Vec<(&Instance, &BlockClass, String)> =
            vec![(&document.root, flat[0].class, document.root.name.clone())];
        while let Some((instance, class, path)) = pending.pop() {
            if let Some(body) = &class.body {
                for index in cdl_model::evaluation_order(class, &document.library)
                    .map_err(|e| Error::Diagnostics(vec![e]))?
                    .into_iter()
                    .rev()
                {
                    let child = &body.instances[index];
                    pending.push((
                        child,
                        &document.library.classes[&child.class],
                        format!("{path}.{}", child.name),
                    ));
                }
                continue;
            }
            let mut io = SlotAlloc::default();
            for port in class.inputs.iter().chain(&class.outputs) {
                let slot = ports[&format!("{path}.{}", port.name)].0;
                if port.direction == Direction::Input {
                    io.bind_input(&port.name, slot);
                } else {
                    io.bind_output(&port.name, slot);
                }
            }
            let config = ConfiguredInstance::new(instance, class, &path);
            if let Some(definition) = registry.get(&class.id) {
                data.periods.extend(definition.event_periods(&config));
            }
            let equations = instance
                .meta
                .annotations
                .get("ace:equations")
                .or_else(|| class.meta.annotations.get("ace:equations"));
            let equations = match equations {
                Some(cdl_model::Annotation::Array(e)) if !e.is_empty() => Some(e.as_slice()),
                _ => None,
            };
            #[cfg(not(feature = "equations"))]
            if equations.is_some() {
                return Err(Error::Diagnostics(vec![Diagnostic::new(
                    &path,
                    DiagnosticCode::InvalidClass,
                    "equation blocks require the equations Cargo feature",
                )]));
            }
            #[cfg(feature = "equations")]
            let equation_kernel = if let Some(equations) = equations {
                // Known builtins retain the original executors' precedence over
                // their documentary equations; free-form extensions evaluate.
                if registry.get(&class.id).is_none() {
                    Some(Instruction::Equations(
                        crate::equations::Equations::build(&config, &io, equations)
                            .map_err(|e| Error::Diagnostics(vec![e]))?,
                    ))
                } else {
                    None
                }
            } else {
                None
            };
            #[cfg(not(feature = "equations"))]
            let equation_kernel: Option<Instruction> = None;
            let kernel = match equation_kernel {
                Some(kernel) => kernel,
                None => Instruction::Builtin(
                    registry
                        .build(&config, &io)
                        .map_err(|e| Error::Diagnostics(vec![e]))?,
                ),
            };
            kernel.initialize(&mut data.slots);
            if kernel.is_constant() {
                #[cfg(feature = "serde")]
                for port in &class.outputs {
                    data.constant_slots
                        .push(ports[&format!("{path}.{}", port.name)].0);
                }
                continue;
            }
            let index = data.kernels.len();
            if kernel.is_stateful() {
                data.stateful.push(index);
            }
            if registry
                .get(&class.id)
                .is_some_and(|d| d.breaks_algebraic_loop())
            {
                data.breakers.push(index);
            }
            if kernel.event_state().is_some() {
                data.previous.push(index);
            }
            data.kernels.push(kernel);
            data.paths.push(path);
        }
        Ok(Self {
            data: Arc::new(data),
        })
    }
}
