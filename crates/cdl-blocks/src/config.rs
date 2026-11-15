use cdl_model::{Annotation, BlockClass, BlockKind, DataType, Instance, Value};
use std::cell::RefCell;

/// A parameter access recorded while building a step, for interface conformance.
#[derive(Clone, Debug, PartialEq)]
pub struct ParameterRead {
    /// Local parameter name.
    pub name: String,
    /// Declared data type, if this is a declared interface parameter.
    pub data_type: Option<DataType>,
    /// Primitive operation used by the implementation.
    pub access: ParameterAccess,
}

/// Primitive interpretation of an implementation parameter read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParameterAccess {
    /// Inspection by a parameter rule.
    Inspect,
    /// Numeric coefficient or duration.
    Number,
    /// Boolean option.
    Boolean,
    /// String or enumeration identifier.
    Text,
}

/// Instance configuration and its class, without a JSON dependency.
pub struct ConfiguredInstance<'a> {
    /// Configured instance.
    pub instance: &'a Instance,
    /// Referenced class.
    pub class: &'a BlockClass,
    /// Diagnostic instance path.
    pub path: &'a str,
    reads: RefCell<Vec<ParameterRead>>,
    effective: RefCell<cdl_model::IndexMap<String, Value>>,
}
impl<'a> ConfiguredInstance<'a> {
    /// Pair an instance with its resolved definition.
    pub fn new(instance: &'a Instance, class: &'a BlockClass, path: &'a str) -> Self {
        Self {
            instance,
            class,
            path,
            reads: RefCell::new(Vec::new()),
            effective: RefCell::new(cdl_model::IndexMap::new()),
        }
    }
    /// Original behavior family, retained explicitly for ACE interface variants.
    pub fn original_type(&self) -> &str {
        let origin = self
            .class
            .meta
            .annotations
            .get("ace:blockType")
            .or_else(|| self.instance.meta.annotations.get("ace:blockType"));
        let name = match origin {
            Some(Annotation::String(s)) => s.as_str(),
            _ => &self.class.id.0,
        };
        name
    }
    /// Short behavior family from its explicit original or standard class name.
    pub fn family(&self) -> &str {
        let name = self.original_type();
        name.rsplit(['.', '#']).next().unwrap_or(name)
    }
    /// Whether compatibility fallbacks, rather than standard declaration defaults, apply.
    pub fn is_legacy(&self) -> bool {
        self.instance
            .meta
            .annotations
            .contains_key("ace:compat:fields")
            || self.class.kind == BlockKind::Extension
    }
    /// Declared connector, including instance metadata/interface overrides.
    pub fn connector(&self, name: &str) -> Option<&cdl_model::Connector> {
        self.instance
            .connectors
            .iter()
            .find(|p| p.name == name)
            .or_else(|| self.class.connector(name))
    }
    /// Parameter reads made by a builder. Stateless kernel stepping does not access this data.
    pub fn parameter_reads(&self) -> Vec<ParameterRead> {
        self.reads.borrow().clone()
    }
    /// Values actually selected by the builder, including compatibility fallbacks.
    pub fn effective_parameters(&self) -> cdl_model::IndexMap<String, Value> {
        self.effective.borrow().clone()
    }
    pub(crate) fn remember(&self, name: &str, value: Value, access: ParameterAccess) {
        if let Some(read) = self.reads.borrow_mut().iter_mut().rfind(|r| r.name == name) {
            read.access = access;
        }
        if self.class.parameters.iter().any(|p| p.name == name) {
            self.effective.borrow_mut().insert(name.into(), value);
        }
    }
    pub(crate) fn value(&self, name: &str) -> Option<ConfigValue<'_>> {
        let declaration = self.class.parameters.iter().find(|p| p.name == name);
        self.reads.borrow_mut().push(ParameterRead {
            name: name.into(),
            data_type: declaration.map(|p| p.data_type.clone()),
            access: ParameterAccess::Inspect,
        });
        if !self.is_legacy() && declaration.is_none() {
            return None;
        }
        self.instance
            .assignments
            .iter()
            .find(|p| p.name == name)
            .map(|p| ConfigValue(&p.value))
            .or_else(|| {
                if self.is_legacy() {
                    None
                } else {
                    declaration
                        .and_then(|p| p.default.as_ref())
                        .map(ConfigValue)
                }
            })
    }
    pub(crate) fn number(&self, name: &str, default: f64) -> f64 {
        self.number_inner(name, default, &mut Vec::new())
    }
    fn number_inner(&self, name: &str, default: f64, active: &mut Vec<String>) -> f64 {
        if active.iter().any(|n| n == name) {
            return default;
        }
        active.push(name.into());
        let value = match self.value(name) {
            Some(ConfigValue(Value::Expr(s))) if s.parse::<f64>().is_err() => {
                self.number_inner(s, default, active)
            }
            Some(value) => value.as_f64().unwrap_or(0.0),
            None => default,
        };
        active.pop();
        let scalar = if self
            .class
            .parameters
            .iter()
            .any(|p| p.name == name && p.data_type == DataType::Integer)
            && value.is_finite()
            && value.fract() == 0.0
            && value >= i64::MIN as f64
            && value < i64::MAX as f64
        {
            Value::Integer(value as i64)
        } else {
            Value::Real(value)
        };
        self.remember(name, scalar, ParameterAccess::Number);
        value
    }
    pub(crate) fn boolean(&self, name: &str, default: bool) -> bool {
        let value = self
            .value(name)
            .and_then(|v| v.as_bool())
            .unwrap_or(default);
        self.remember(name, Value::Boolean(value), ParameterAccess::Boolean);
        value
    }
}
#[derive(Clone, Copy)]
pub(crate) struct ConfigValue<'a>(pub &'a Value);
impl<'a> ConfigValue<'a> {
    pub(crate) fn as_f64(self) -> Option<f64> {
        match self.0 {
            Value::Real(v) => Some(*v),
            Value::Integer(v) => Some(*v as f64),
            Value::Expr(s) => s.parse().ok(),
            _ => None,
        }
    }
    pub(crate) fn as_bool(self) -> Option<bool> {
        match self.0 {
            Value::Boolean(v) => Some(*v),
            _ => None,
        }
    }
    pub(crate) fn as_str(self) -> Option<&'a str> {
        match self.0 {
            Value::String(v) | Value::Expr(v) | Value::Enum(v) => Some(v),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub(crate) enum PidControllerType {
    P,
    Pi,
    Pd,
    Pid,
}
impl PidControllerType {
    pub(crate) fn with_integral(self) -> bool {
        matches!(self, Self::Pi | Self::Pid)
    }
    pub(crate) fn with_derivative(self) -> bool {
        matches!(self, Self::Pd | Self::Pid)
    }
}
pub(crate) fn pid_controller_type(config: &ConfiguredInstance<'_>) -> Option<PidControllerType> {
    let raw = config.value("controllerType");
    let mode = if let Some(value) = raw {
        value.as_str()?.rsplit('.').next()?
    } else {
        "PI"
    };
    let result = match mode {
        "P" => PidControllerType::P,
        "PI" => PidControllerType::Pi,
        "PD" => PidControllerType::Pd,
        "PID" => PidControllerType::Pid,
        _ => return None,
    };
    let expression = match config
        .class
        .parameters
        .iter()
        .find(|p| p.name == "controllerType")
        .map(|p| &p.data_type)
    {
        Some(DataType::Enumeration(id)) => format!(
            "{}.{}",
            id.0.strip_prefix("http://data.ashrae.org/S231#")
                .unwrap_or(&id.0),
            mode
        ),
        _ => mode.to_owned(),
    };
    config.remember(
        "controllerType",
        Value::Expr(expression),
        ParameterAccess::Text,
    );
    Some(result)
}
