use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable machine-readable structural and adapter diagnostic codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum DiagnosticCode {
    /// Unparseable source representation.
    InvalidDocument,
    /// Referenced class is absent.
    UnknownClass,
    /// Name has no unique class resolution.
    AmbiguousClass,
    /// Duplicate class member or instance name.
    DuplicateName,
    /// Empty or invalid name.
    InvalidName,
    /// Class category and body disagree.
    InvalidClass,
    /// Class instantiation recursively contains itself.
    RecursiveClass,
    /// Wire refers to an absent child or connector.
    MissingEndpoint,
    /// Wire does not join a source and sink.
    InvalidDirection,
    /// Incompatible data types or array dimensions.
    TypeMismatch,
    /// More than one driver for the same connector.
    MultipleDrivers,
    /// Required child input or composite output has no driver.
    UndrivenInput,
    /// Algebraic dependencies cannot be ordered.
    AlgebraicCycle,
    /// Parameter binding does not reference a declaration.
    UnknownParameter,
    /// A value is inconsistent with its declared type.
    InvalidValue,
}

/// Diagnostic with a stable code and instance/declaration path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Diagnostic {
    /// Instance or declaration path.
    pub path: String,
    /// Stable code.
    pub code: DiagnosticCode,
    /// Human-readable explanation.
    pub message: String,
}
impl Diagnostic {
    /// Construct a diagnostic.
    pub fn new(path: impl Into<String>, code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            code,
            message: message.into(),
        }
    }
}
impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}
impl std::error::Error for Diagnostic {}
