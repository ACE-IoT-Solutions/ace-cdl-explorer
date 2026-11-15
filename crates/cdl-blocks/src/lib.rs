//! CDL elementary definitions and stateful kernels over numeric slots.
//!
//! Hosts own JSON encoding and simulation. All production dependencies are in cdl-model.

#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

mod catalog;
pub use catalog::legacy_catalog_classes;
mod config;
mod conformance;
mod defaults;
mod kernel;
mod registry;
mod rules;
mod slots;

pub use config::{ConfiguredInstance, ParameterAccess, ParameterRead};
pub use defaults::implicit_assignment_unchanged;
pub use kernel::{BlockMemory, BlockSnapshot, Kernel};
pub use registry::{BlockImpl, Definition, Registry, LEGACY_NAMES};
pub use slots::{Slot, SlotAlloc};
use std::fmt;

/// Validated duration for a single evaluation, including initialization at dt=0.
#[derive(Clone, Copy, Debug)]
pub struct StepCtx {
    dt: f64,
}
impl StepCtx {
    /// Reject negative and nonfinite durations before entering the kernel loop.
    pub fn new(dt: f64) -> Result<Self, KernelError> {
        if !dt.is_finite() || dt < 0.0 {
            return Err(KernelError(
                "step duration must be finite and nonnegative".into(),
            ));
        }
        Ok(Self { dt })
    }
    /// Duration in seconds.
    pub fn dt(&self) -> f64 {
        self.dt
    }
}

/// Runtime input or step-context error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KernelError(pub String);
impl From<&str> for KernelError {
    fn from(s: &str) -> Self {
        Self(s.into())
    }
}
impl fmt::Display for KernelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for KernelError {}

/// Execution contract, including simultaneous read/capture for cycle breakers.
pub trait Step {
    /// Initialize held outputs before the first evaluation.
    fn initialize(&self, slots: &mut [f64]);
    /// Whether event iteration must checkpoint this step.
    fn is_stateful(&self) -> bool;
    /// Publish stored outputs before ordered evaluation.
    fn prepare(&mut self, slots: &mut [f64], ctx: &StepCtx);
    /// Evaluate this step in topological order.
    fn step(&mut self, slots: &mut [f64], ctx: &StepCtx) -> Result<(), KernelError>;
    /// Capture final inputs after every step has evaluated.
    fn commit(&mut self, slots: &[f64]);
}
