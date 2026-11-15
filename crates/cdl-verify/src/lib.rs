//! Portable suite loading, input replay and absolute-tolerance funnel comparison.
#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

mod funnel;
mod replay;
mod suite;
pub use funnel::{compare, Comparison, Sample, Trace};
pub use replay::{
    input_scalar, load_document, read_csv, CdlImplementation, Implementation, InputRow, Trend,
};
pub use suite::{Case, CaseResult, Suite};

/// A validation, I/O, compilation or execution failure with its source explanation.
pub type Error = Box<dyn std::error::Error>;
/// Result of a verification operation.
pub type Result<T> = std::result::Result<T, Error>;
