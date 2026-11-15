//! CDL classes, instances and wiring, independent of serialization formats and execution.
//!
//! The vocabulary follows [CXF §8.2](https://obc.lbl.gov/specification/cxf.html#classes-and-properties).
//! This crate preserves expressions and annotations; it does not evaluate control logic.

#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

mod diagnostic;
mod path;
mod types;
mod validate;

pub use diagnostic::*;
pub use indexmap::IndexMap;
pub use path::*;
pub use types::*;
pub use validate::{evaluation_order, validate};
