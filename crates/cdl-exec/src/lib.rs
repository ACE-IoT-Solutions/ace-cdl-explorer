//! A compiled, format-independent CDL execution engine.
#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

mod alignment;
pub use alignment::aligned_step;
mod compile;
#[cfg(feature = "equations")]
mod equations;
mod instruction;
#[cfg(feature = "serde")]
mod snapshot;
mod state;
#[cfg(feature = "serde")]
pub use snapshot::{SnapshotError, StateSnapshot};

use cdl_model::{DataType, Diagnostic};
use instruction::Instruction;
pub use state::State;
use std::{collections::HashMap, fmt, sync::Arc};

/// A scalar value crossing the host boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scalar {
    /// Real-valued signal.
    Real(f64),
    /// Integral signal.
    Integer(i64),
    /// Boolean signal.
    Boolean(bool),
}

/// Compilation, binding or evaluation failure.
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// Invalid structure or unsupported implementation/configuration.
    Diagnostics(Vec<Diagnostic>),
    /// An unknown input, output or internal connector path.
    UnknownPort(String),
    /// A handle belongs to another program or cannot be written.
    InvalidHandle,
    /// A host value does not match the declared connector type.
    TypeMismatch,
    /// A host real input is not finite.
    InvalidValue,
    /// At least one root input has never been assigned.
    MissingInput(String),
    /// The step duration is negative or nonfinite.
    InvalidStep,
    /// Timed events cannot be aligned within the building simulator's resolution.
    InvalidAlignment(String),
    /// An elementary implementation failed at its instance path.
    Kernel {
        /// Full instance path.
        path: String,
        /// Kernel explanation.
        message: String,
    },
    /// Boolean Pre iteration exceeded the production limit of 32 attempts.
    NonConvergent,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Diagnostics(d) => write!(
                f,
                "{}",
                d.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
            Self::UnknownPort(p) => write!(f, "unknown port {p}"),
            Self::InvalidHandle => f.write_str("invalid port handle for this program or operation"),
            Self::TypeMismatch => f.write_str("input value does not match its declared type"),
            Self::InvalidValue => f.write_str("input must be finite"),
            Self::MissingInput(p) => write!(f, "Missing input {p}"),
            Self::InvalidStep => f.write_str("step duration must be finite and nonnegative"),
            Self::InvalidAlignment(message) => f.write_str(message),
            Self::Kernel { path, message } => write!(f, "{path}: {message}"),
            Self::NonConvergent => f.write_str("Boolean Pre event iteration did not converge"),
        }
    }
}
impl std::error::Error for Error {}

/// A pre-resolved connector. Handles are scoped to the program that created them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortHandle {
    program: u64,
    slot: usize,
    data_type: DataType,
    input: Option<usize>,
}
impl PortHandle {
    /// Declared scalar signal type.
    pub fn data_type(&self) -> &DataType {
        &self.data_type
    }
}

struct Data {
    id: u64,
    #[cfg(feature = "serde")]
    fingerprint: [u8; 32],
    #[cfg(feature = "serde")]
    constant_slots: Vec<usize>,
    #[cfg(feature = "serde")]
    signal_paths: std::collections::BTreeMap<String, usize>,
    slots: Vec<f64>,
    kernels: Vec<Instruction>,
    paths: Vec<String>,
    stateful: Vec<usize>,
    breakers: Vec<usize>,
    previous: Vec<usize>,
    periods: Vec<f64>,
    inputs: HashMap<String, PortHandle>,
    outputs: HashMap<String, PortHandle>,
    traces: HashMap<String, PortHandle>,
}

/// Immutable program shared by independently instantiated states.
#[derive(Clone)]
pub struct Program {
    data: Arc<Data>,
}
impl Program {
    /// Align a requested whole-second building step with all sampled/timed events.
    /// Retains the existing simulator's 15-second minimum and rejects fractional
    /// or smaller event intervals instead of silently skipping transitions.
    pub fn aligned_step(&self, requested_seconds: u64) -> Result<u64, Error> {
        alignment::align_periods(requested_seconds, self.data.periods.iter().copied())
    }
    /// Resolve a root input once for repeated writes without string lookups.
    pub fn input(&self, name: &str) -> Result<PortHandle, Error> {
        self.data
            .inputs
            .get(name)
            .cloned()
            .ok_or_else(|| Error::UnknownPort(name.into()))
    }
    /// Resolve a root output once for repeated reads without string lookups.
    pub fn output(&self, name: &str) -> Result<PortHandle, Error> {
        self.data
            .outputs
            .get(name)
            .cloned()
            .ok_or_else(|| Error::UnknownPort(name.into()))
    }
    /// Resolve any internal connector by its full `root.child.port` path.
    /// Legacy `composite|child.port`, `|edge.N`, and `|output.name` paths also work.
    pub fn trace(&self, path: &str) -> Result<PortHandle, Error> {
        self.data
            .traces
            .get(path)
            .cloned()
            .ok_or_else(|| Error::UnknownPort(path.into()))
    }
    /// Create independent initialized execution state.
    pub fn instantiate(&self) -> State {
        State::new(self.data.clone())
    }
}
