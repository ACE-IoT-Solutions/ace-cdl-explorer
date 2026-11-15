use crate::{ConfiguredInstance, Kernel, SlotAlloc};
use cdl_model::IndexMap;
use cdl_model::{Annotation, BlockClass, BlockKind, ClassId, Diagnostic, DiagnosticCode, Library};

/// The 43 behavior families supported by the original runtime.
pub const LEGACY_NAMES: &[&str] = &[
    "Constant",
    "Add",
    "AddParameter",
    "Min",
    "Max",
    "Line",
    "BooleanToInteger",
    "RealToInteger",
    "BooleanToReal",
    "MultiplyByParameter",
    "Limiter",
    "LimPID",
    "PIDWithReset",
    "Filter",
    "LimRateLimiter",
    "Hysteresis",
    "Switch",
    "Gain",
    "Greater",
    "GreaterThreshold",
    "LessThreshold",
    "Equal",
    "Less",
    "Subtract",
    "Multiply",
    "Divide",
    "And",
    "Or",
    "Not",
    "Delay",
    "TrueDelay",
    "TrueFalseHold",
    "Latch",
    "UnitDelay",
    "Sampler",
    "Pre",
    "TriggeredSampler",
    "TriggeredMovingMean",
    "SampleTrigger",
    "Edge",
    "FallingEdge",
    "Timer",
    "IntegerToReal",
];
/// An elementary implementation: interface, parameter checks and executable behavior.
pub trait BlockImpl: Send + Sync {
    /// Canonical implementation identifier.
    fn class(&self) -> &ClassId;
    /// Check implementation-specific parameter rules.
    fn check(&self, instance: &ConfiguredInstance<'_>, out: &mut Vec<Diagnostic>);
    /// Bind and construct an independent kernel.
    fn build(
        &self,
        instance: &ConfiguredInstance<'_>,
        io: &SlotAlloc,
    ) -> Result<Kernel, Diagnostic>;
    /// Whether this implementation breaks algebraic loops.
    fn breaks_algebraic_loop(&self) -> bool;
    /// Event periods and phases used to align integration steps.
    fn event_periods(&self, instance: &ConfiguredInstance<'_>) -> Vec<f64>;
}
/// One behavior definition, reused across typed and legacy interfaces.
#[derive(Clone, Debug)]
pub struct Definition {
    id: ClassId,
    name: &'static str,
}
impl Definition {
    /// Behavior family name.
    pub fn name(&self) -> &'static str {
        self.name
    }
}
impl BlockImpl for Definition {
    fn class(&self) -> &ClassId {
        &self.id
    }
    fn check(&self, instance: &ConfiguredInstance<'_>, out: &mut Vec<Diagnostic>) {
        crate::rules::check(instance, out)
    }
    fn build(
        &self,
        instance: &ConfiguredInstance<'_>,
        io: &SlotAlloc,
    ) -> Result<Kernel, Diagnostic> {
        if instance.family() != self.name
            || !crate::conformance::supported(instance.class, self.name)
        {
            return Err(Diagnostic::new(
                instance.path,
                DiagnosticCode::InvalidClass,
                "class interface does not match its registered implementation",
            ));
        }
        let mut errors = Vec::new();
        self.check(instance, &mut errors);
        if let Some(error) = errors.into_iter().next() {
            return Err(error);
        }
        let kernel = crate::kernel::build(instance, io)?;
        if !instance.is_legacy() {
            if let Some(read) = instance
                .parameter_reads()
                .into_iter()
                .find(|p| p.data_type.is_none())
            {
                return Err(Diagnostic::new(
                    instance.path,
                    DiagnosticCode::InvalidClass,
                    format!("implementation reads undeclared parameter {}", read.name),
                ));
            }
        }
        Ok(kernel)
    }
    fn breaks_algebraic_loop(&self) -> bool {
        matches!(self.name, "UnitDelay" | "Pre")
    }
    fn event_periods(&self, instance: &ConfiguredInstance<'_>) -> Vec<f64> {
        match self.name {
            "UnitDelay" | "Sampler" => vec![instance.number("samplePeriod", 1.0)],
            "SampleTrigger" => vec![
                instance.number("period", 1.0),
                instance.number("shift", 0.0).abs(),
            ],
            "TrueDelay" => vec![instance.number("delayTime", 0.0)],
            "TrueFalseHold" => {
                let duration = instance.number("trueHoldDuration", 0.0);
                vec![duration, instance.number("falseHoldDuration", duration)]
            }
            _ => vec![],
        }
    }
}
/// Registry over an authoritative caller-supplied model library.
///
/// Interface data is not duplicated in handwritten block definitions. Hosts load
/// the pinned core through the format crate and may supply user classes alongside it.
pub struct Registry {
    library: Library,
    definitions: Vec<Definition>,
    entries: IndexMap<ClassId, Definition>,
}
impl Registry {
    /// Load authoritative interfaces and the 43 legacy behavior families.
    pub fn new(library: Library) -> Self {
        let definitions: Vec<_> = LEGACY_NAMES
            .iter()
            .map(|name| Definition {
                id: format!(
                    "https://ace-iot-solutions.github.io/ace-cdl-explorer/cxf#legacy.{name}"
                )
                .into(),
                name,
            })
            .collect();
        let entries = library
            .classes
            .values()
            .filter(|class| class.kind != BlockKind::Composite)
            .filter_map(|class| {
                let origin = if class.id.0.starts_with("http://data.ashrae.org/S231#") {
                    class.id.0.as_str()
                } else if class
                    .id
                    .0
                    .starts_with("https://ace-iot-solutions.github.io/ace-cdl-explorer/cxf#")
                {
                    match class.meta.annotations.get("ace:blockType") {
                        Some(Annotation::String(origin)) => origin,
                        _ => return None,
                    }
                } else {
                    return None;
                };
                let family = origin.rsplit(['.', '#']).next()?;
                let definition = definitions
                    .iter()
                    .find(|definition| definition.name == family)?;
                if !crate::conformance::supported(class, definition.name) {
                    return None;
                }
                Some((
                    class.id.clone(),
                    Definition {
                        id: class.id.clone(),
                        name: definition.name,
                    },
                ))
            })
            .collect();
        Self {
            library,
            definitions,
            entries,
        }
    }
    /// Authoritative interface library.
    pub fn library(&self) -> &Library {
        &self.library
    }
    /// Resolve one compatibility behavior family uniquely.
    pub fn legacy(&self, name: &str) -> Option<&Definition> {
        self.definitions.iter().find(|d| d.name == name)
    }
    /// Resolve a registered class IRI without guessing from arbitrary user class names.
    pub fn get(&self, id: &ClassId) -> Option<&Definition> {
        self.entries.get(id)
    }
    /// Build a kernel from its resolved class and configuration.
    pub fn build(
        &self,
        instance: &ConfiguredInstance<'_>,
        io: &SlotAlloc,
    ) -> Result<Kernel, Diagnostic> {
        self.get(&instance.class.id)
            .ok_or_else(|| {
                Diagnostic::new(
                    instance.path,
                    DiagnosticCode::UnknownClass,
                    format!("unsupported class {}", instance.class.id),
                )
            })?
            .build(instance, io)
    }
    /// Serializable catalog. Hosts own JSON encoding to preserve the dependency boundary.
    pub fn catalog(&self) -> Library {
        let mut catalog = self.library.clone();
        for (id, class) in &mut catalog.classes {
            let local = id.0.rsplit('#').next().unwrap_or(&id.0);
            if class.meta.label.is_none() {
                class.meta.label = Some(local.rsplit('.').next().unwrap_or(local).into());
            }
            if let Some((group, _)) = local.rsplit_once('.') {
                class
                    .meta
                    .annotations
                    .insert("ace:group".into(), Annotation::String(group.into()));
            }
            class.meta.annotations.insert(
                "ace:implemented".into(),
                Annotation::Boolean(self.entries.contains_key(id)),
            );
        }
        catalog
    }
    /// Standard class interfaces with their implementation availability.
    pub fn standard_classes(&self) -> impl Iterator<Item = (&BlockClass, bool)> {
        self.library
            .classes
            .values()
            .filter(|c| c.kind == BlockKind::Elementary)
            .map(|c| (c, self.get(&c.id).is_some()))
    }
}
