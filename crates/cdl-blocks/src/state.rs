//! Versioned block memory, independent of slot layout and serialization format.
use super::{Kernel, Op};
use crate::KernelError;
use std::collections::VecDeque;

/// Portable memory for a configured block. Signal values are owned by the executor.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct BlockSnapshot {
    /// State schema version; currently one.
    pub version: u32,
    /// Opaque versioned configuration identity, excluding memory and slot addresses.
    pub configuration: String,
    /// Family-specific dynamic memory.
    pub memory: BlockMemory,
}
/// Dynamic memory only; bindings, coefficients and connector slots are excluded.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "kind", deny_unknown_fields))]
pub enum BlockMemory {
    /// A kernel without internal memory (held output values remain executor signals).
    Stateless,
    /// Sampled transport delay memory.
    UnitDelay {
        /// Elapsed sample phase in seconds.
        #[cfg_attr(feature = "serde", serde(with = "wire::number"))]
        elapsed: f64,
        /// Stored output.
        #[cfg_attr(feature = "serde", serde(with = "wire::number"))]
        state: f64,
        /// Previous input.
        #[cfg_attr(feature = "serde", serde(with = "wire::number"))]
        last_input: f64,
        /// Whether initialization has completed.
        initialized: bool,
        /// Whether this step sampled its input.
        sampled_now: bool,
    },
    /// Previous Boolean event value, represented as a zero/one slot.
    Pre {
        /// Stored Boolean value.
        #[cfg_attr(feature = "serde", serde(with = "wire::number"))]
        state: f64,
    },
    /// Dynamic memory for Filter.
    Filter {
        /// Stored old.
        #[cfg_attr(feature = "serde", serde(with = "wire::optional"))]
        old: Option<f64>,
    },
    /// Dynamic memory for Rate.
    Rate {
        /// Stored old.
        #[cfg_attr(feature = "serde", serde(with = "wire::optional"))]
        old: Option<f64>,
    },
    /// Dynamic memory for Hysteresis.
    Hysteresis {
        /// Stored old.
        old: bool,
    },
    /// Dynamic memory for Pi.
    Pi {
        /// Stored integral.
        #[cfg_attr(feature = "serde", serde(with = "wire::number"))]
        integral: f64,
    },
    /// Dynamic memory for PiWithReset.
    PiWithReset {
        /// Stored derivative x.
        #[cfg_attr(feature = "serde", serde(with = "wire::optional"))]
        derivative_x: Option<f64>,
        /// Stored integral.
        #[cfg_attr(feature = "serde", serde(with = "wire::number"))]
        integral: f64,
        /// Stored previous trigger.
        previous_trigger: bool,
        /// Stored trigger initialized.
        trigger_initialized: bool,
    },
    /// Dynamic memory for Greater.
    Greater {
        /// Stored old.
        old: bool,
    },
    /// Dynamic memory for GreaterThreshold.
    GreaterThreshold {
        /// Stored old.
        old: bool,
    },
    /// Dynamic memory for LessThreshold.
    LessThreshold {
        /// Stored old.
        old: bool,
    },
    /// Dynamic memory for Less.
    Less {
        /// Stored old.
        old: bool,
    },
    /// Dynamic memory for Delay.
    Delay {
        /// Stored old.
        #[cfg_attr(feature = "serde", serde(with = "wire::optional"))]
        old: Option<f64>,
    },
    /// Dynamic memory for TrueDelay.
    TrueDelay {
        /// Stored elapsed.
        #[cfg_attr(feature = "serde", serde(with = "wire::optional"))]
        elapsed: Option<f64>,
        /// Stored old active.
        old_active: Option<bool>,
    },
    /// Dynamic memory for Latch.
    Latch {
        /// Stored old set.
        old_set: Option<bool>,
        /// Stored old clear.
        old_clear: Option<bool>,
        /// Stored state.
        state: bool,
    },
    /// Dynamic memory for TrueFalseHold.
    TrueFalseHold {
        /// Stored old input.
        old_input: bool,
        /// Stored state.
        state: bool,
        /// Stored age.
        #[cfg_attr(feature = "serde", serde(with = "wire::number"))]
        age: f64,
        /// Stored initialized.
        initialized: bool,
    },
    /// Dynamic memory for TriggeredSampler.
    TriggeredSampler {
        /// Stored old trigger.
        old_trigger: bool,
    },
    /// Dynamic memory for Sampler.
    Sampler {
        /// Stored elapsed.
        #[cfg_attr(feature = "serde", serde(with = "wire::number"))]
        elapsed: f64,
        /// Stored initialized.
        initialized: bool,
    },
    /// Dynamic memory for TriggeredMovingMean.
    TriggeredMovingMean {
        /// Stored old trigger.
        old_trigger: bool,
        /// Stored initialized.
        initialized: bool,
        /// Stored samples.
        #[cfg_attr(feature = "serde", serde(with = "wire::queue"))]
        samples: VecDeque<f64>,
    },
    /// Dynamic memory for SampleTrigger.
    SampleTrigger {
        /// Stored clock.
        #[cfg_attr(feature = "serde", serde(with = "wire::number"))]
        clock: f64,
        /// Stored initialized.
        initialized: bool,
    },
    /// Dynamic memory for Edge.
    Edge {
        /// Stored old.
        old: bool,
    },
    /// Dynamic memory for Timer.
    Timer {
        /// Stored old active.
        old_active: bool,
        /// Stored elapsed.
        #[cfg_attr(feature = "serde", serde(with = "wire::number"))]
        elapsed: f64,
    },
}
impl BlockSnapshot {
    /// Check schema and intrinsic memory validity. Configuration compatibility and
    /// configured buffer capacity are additionally checked by Kernel::restore.
    pub fn validate(&self) -> Result<(), KernelError> {
        if self.version != 1 || !self.memory.valid() {
            return Err("invalid block snapshot schema or memory".into());
        }
        Ok(())
    }
}
impl Kernel {
    /// Export all internal memory, including comparators omitted from event rollback.
    pub fn snapshot(&self) -> BlockSnapshot {
        let memory = match &self.op {
            Op::UnitDelay(s) => BlockMemory::UnitDelay {
                elapsed: s.elapsed,
                state: s.state,
                last_input: s.last_input,
                initialized: s.initialized,
                sampled_now: s.sampled_now,
            },
            Op::Pre(s) => BlockMemory::Pre { state: s.state },
            Op::Filter { old, .. } => BlockMemory::Filter { old: *old },
            Op::Rate { old, .. } => BlockMemory::Rate { old: *old },
            Op::Hysteresis { old, .. } => BlockMemory::Hysteresis { old: *old },
            Op::Pi { integral, .. } => BlockMemory::Pi {
                integral: *integral,
            },
            Op::PiWithReset {
                derivative_x,
                integral,
                previous_trigger,
                trigger_initialized,
                ..
            } => BlockMemory::PiWithReset {
                derivative_x: *derivative_x,
                integral: *integral,
                previous_trigger: *previous_trigger,
                trigger_initialized: *trigger_initialized,
            },
            Op::Greater { old, .. } => BlockMemory::Greater { old: *old },
            Op::GreaterThreshold { old, .. } => BlockMemory::GreaterThreshold { old: *old },
            Op::LessThreshold { old, .. } => BlockMemory::LessThreshold { old: *old },
            Op::Less { old, .. } => BlockMemory::Less { old: *old },
            Op::Delay { old, .. } => BlockMemory::Delay { old: *old },
            Op::TrueDelay {
                elapsed,
                old_active,
                ..
            } => BlockMemory::TrueDelay {
                elapsed: *elapsed,
                old_active: *old_active,
            },
            Op::Latch {
                old_set,
                old_clear,
                state,
                ..
            } => BlockMemory::Latch {
                old_set: *old_set,
                old_clear: *old_clear,
                state: *state,
            },
            Op::TrueFalseHold {
                old_input,
                state,
                age,
                initialized,
                ..
            } => BlockMemory::TrueFalseHold {
                old_input: *old_input,
                state: *state,
                age: *age,
                initialized: *initialized,
            },
            Op::TriggeredSampler { old_trigger, .. } => BlockMemory::TriggeredSampler {
                old_trigger: *old_trigger,
            },
            Op::Sampler {
                elapsed,
                initialized,
                ..
            } => BlockMemory::Sampler {
                elapsed: *elapsed,
                initialized: *initialized,
            },
            Op::TriggeredMovingMean {
                old_trigger,
                initialized,
                samples,
                ..
            } => BlockMemory::TriggeredMovingMean {
                old_trigger: *old_trigger,
                initialized: *initialized,
                samples: samples.clone(),
            },
            Op::SampleTrigger {
                clock, initialized, ..
            } => BlockMemory::SampleTrigger {
                clock: *clock,
                initialized: *initialized,
            },
            Op::Edge { old, .. } => BlockMemory::Edge { old: *old },
            Op::Timer {
                old_active,
                elapsed,
                ..
            } => BlockMemory::Timer {
                old_active: *old_active,
                elapsed: *elapsed,
            },
            _ => BlockMemory::Stateless,
        };
        BlockSnapshot {
            version: 1,
            configuration: self.state_configuration(),
            memory,
        }
    }
    fn state_configuration(&self) -> String {
        match &self.op {
            Op::UnitDelay(s) => format!("UnitDelay:{:?}", s.period),
            Op::Pre(_) => "Pre".into(),
            Op::Constant { value, .. } => format!("Constant:{:?}", (value,)),
            Op::Add { k1, k2, .. } => format!("Add:{:?}", (k1, k2,)),
            Op::AddParameter { p, .. } => format!("AddParameter:{:?}", (p,)),
            Op::Min { .. } => "Min".into(),
            Op::Max { .. } => "Max".into(),
            Op::Line {
                limit_below,
                limit_above,
                ..
            } => format!("Line:{:?}", (limit_below, limit_above,)),
            Op::BooleanToInteger {
                integer_true,
                integer_false,
                ..
            } => format!("BooleanToInteger:{:?}", (integer_true, integer_false,)),
            Op::RealToInteger { .. } => "RealToInteger".into(),
            Op::BooleanToReal {
                real_true,
                real_false,
                ..
            } => format!("BooleanToReal:{:?}", (real_true, real_false,)),
            Op::IntegerToReal { .. } => "IntegerToReal".into(),
            Op::Gain { k, .. } => format!("Gain:{:?}", (k,)),
            Op::Limit { low, high, .. } => format!("Limit:{:?}", (low, high,)),
            Op::Filter { alpha_time, .. } => format!("Filter:{:?}", (alpha_time,)),
            Op::Rate { rise, fall, .. } => format!("Rate:{:?}", (rise, fall,)),
            Op::Hysteresis { low, high, .. } => format!("Hysteresis:{:?}", (low, high,)),
            Op::Switch { .. } => "Switch".into(),
            Op::Pi {
                reverse,
                k,
                ti,
                low,
                high,
                ..
            } => format!("Pi:{:?}", (reverse, k, ti, low, high,)),
            Op::PiWithReset {
                reverse,
                k,
                ti,
                scale,
                low,
                high,
                ni,
                controller_type,
                td,
                nd,
                derivative_start,
                reset,
                ..
            } => format!(
                "PiWithReset:{:?}",
                (
                    reverse,
                    k,
                    ti,
                    scale,
                    low,
                    high,
                    ni,
                    controller_type,
                    td,
                    nd,
                    derivative_start,
                    reset,
                )
            ),
            Op::Greater { hysteresis, .. } => format!("Greater:{:?}", (hysteresis,)),
            Op::GreaterThreshold {
                threshold,
                hysteresis,
                ..
            } => format!("GreaterThreshold:{:?}", (threshold, hysteresis,)),
            Op::LessThreshold {
                threshold,
                hysteresis,
                ..
            } => format!("LessThreshold:{:?}", (threshold, hysteresis,)),
            Op::Equal { .. } => "Equal".into(),
            Op::Less { hysteresis, .. } => format!("Less:{:?}", (hysteresis,)),
            Op::Subtract { .. } => "Subtract".into(),
            Op::Multiply { .. } => "Multiply".into(),
            Op::Divide { .. } => "Divide".into(),
            Op::And { .. } => "And".into(),
            Op::Or { .. } => "Or".into(),
            Op::Not { .. } => "Not".into(),
            Op::Delay { .. } => "Delay".into(),
            Op::TrueDelay {
                delay,
                delay_on_init,
                ..
            } => format!("TrueDelay:{:?}", (delay, delay_on_init,)),
            Op::Latch { .. } => "Latch".into(),
            Op::TrueFalseHold {
                true_duration,
                false_duration,
                ..
            } => format!("TrueFalseHold:{:?}", (true_duration, false_duration,)),
            Op::TriggeredSampler { .. } => "TriggeredSampler".into(),
            Op::Sampler { period, .. } => format!("Sampler:{:?}", (period,)),
            Op::TriggeredMovingMean { capacity, .. } => {
                format!("TriggeredMovingMean:{:?}", (capacity,))
            }
            Op::SampleTrigger { period, shift, .. } => {
                format!("SampleTrigger:{:?}", (period, shift,))
            }
            Op::Edge { falling, .. } => format!("Edge:{:?}", (falling,)),
            Op::Timer { threshold, .. } => format!("Timer:{:?}", (threshold,)),
        }
    }
    /// Restore matching memory after validating schema, configuration and all values.
    /// On failure the kernel is unchanged. Restore connector values separately.
    pub fn restore(&mut self, snapshot: &BlockSnapshot) -> Result<(), KernelError> {
        if snapshot.version != 1
            || snapshot.configuration != self.state_configuration()
            || !snapshot.memory.valid()
        {
            return Err("incompatible or invalid block snapshot".into());
        }
        let stateless = matches!(self.snapshot().memory, BlockMemory::Stateless);
        match (&mut self.op, &snapshot.memory) {
            (
                Op::UnitDelay(s),
                BlockMemory::UnitDelay {
                    elapsed,
                    state,
                    last_input,
                    initialized,
                    sampled_now,
                },
            ) => {
                s.elapsed = *elapsed;
                s.state = *state;
                s.last_input = *last_input;
                s.initialized = *initialized;
                s.sampled_now = *sampled_now;
            }
            (Op::Pre(s), BlockMemory::Pre { state }) => {
                s.state = *state;
            }
            (
                Op::Filter {
                    old: target_old, ..
                },
                BlockMemory::Filter { old },
            ) => {
                *target_old = *old;
            }
            (
                Op::Rate {
                    old: target_old, ..
                },
                BlockMemory::Rate { old },
            ) => {
                *target_old = *old;
            }
            (
                Op::Hysteresis {
                    old: target_old, ..
                },
                BlockMemory::Hysteresis { old },
            ) => {
                *target_old = *old;
            }
            (
                Op::Pi {
                    integral: target_integral,
                    ..
                },
                BlockMemory::Pi { integral },
            ) => {
                *target_integral = *integral;
            }
            (
                Op::PiWithReset {
                    derivative_x: target_derivative_x,
                    integral: target_integral,
                    previous_trigger: target_previous_trigger,
                    trigger_initialized: target_trigger_initialized,
                    ..
                },
                BlockMemory::PiWithReset {
                    derivative_x,
                    integral,
                    previous_trigger,
                    trigger_initialized,
                },
            ) => {
                *target_derivative_x = *derivative_x;
                *target_integral = *integral;
                *target_previous_trigger = *previous_trigger;
                *target_trigger_initialized = *trigger_initialized;
            }
            (
                Op::Greater {
                    old: target_old, ..
                },
                BlockMemory::Greater { old },
            ) => {
                *target_old = *old;
            }
            (
                Op::GreaterThreshold {
                    old: target_old, ..
                },
                BlockMemory::GreaterThreshold { old },
            ) => {
                *target_old = *old;
            }
            (
                Op::LessThreshold {
                    old: target_old, ..
                },
                BlockMemory::LessThreshold { old },
            ) => {
                *target_old = *old;
            }
            (
                Op::Less {
                    old: target_old, ..
                },
                BlockMemory::Less { old },
            ) => {
                *target_old = *old;
            }
            (
                Op::Delay {
                    old: target_old, ..
                },
                BlockMemory::Delay { old },
            ) => {
                *target_old = *old;
            }
            (
                Op::TrueDelay {
                    elapsed: target_elapsed,
                    old_active: target_old_active,
                    ..
                },
                BlockMemory::TrueDelay {
                    elapsed,
                    old_active,
                },
            ) => {
                *target_elapsed = *elapsed;
                *target_old_active = *old_active;
            }
            (
                Op::Latch {
                    old_set: target_old_set,
                    old_clear: target_old_clear,
                    state: target_state,
                    ..
                },
                BlockMemory::Latch {
                    old_set,
                    old_clear,
                    state,
                },
            ) => {
                *target_old_set = *old_set;
                *target_old_clear = *old_clear;
                *target_state = *state;
            }
            (
                Op::TrueFalseHold {
                    old_input: target_old_input,
                    state: target_state,
                    age: target_age,
                    initialized: target_initialized,
                    ..
                },
                BlockMemory::TrueFalseHold {
                    old_input,
                    state,
                    age,
                    initialized,
                },
            ) => {
                *target_old_input = *old_input;
                *target_state = *state;
                *target_age = *age;
                *target_initialized = *initialized;
            }
            (
                Op::TriggeredSampler {
                    old_trigger: target_old_trigger,
                    ..
                },
                BlockMemory::TriggeredSampler { old_trigger },
            ) => {
                *target_old_trigger = *old_trigger;
            }
            (
                Op::Sampler {
                    elapsed: target_elapsed,
                    initialized: target_initialized,
                    ..
                },
                BlockMemory::Sampler {
                    elapsed,
                    initialized,
                },
            ) => {
                *target_elapsed = *elapsed;
                *target_initialized = *initialized;
            }
            (
                Op::TriggeredMovingMean {
                    old_trigger: target_old_trigger,
                    initialized: target_initialized,
                    samples: target_samples,
                    capacity,
                    ..
                },
                BlockMemory::TriggeredMovingMean {
                    old_trigger,
                    initialized,
                    samples,
                },
            ) => {
                if samples.len() > *capacity {
                    return Err("moving mean snapshot exceeds configured capacity".into());
                }
                *target_old_trigger = *old_trigger;
                *target_initialized = *initialized;
                *target_samples = samples.clone();
            }
            (
                Op::SampleTrigger {
                    clock: target_clock,
                    initialized: target_initialized,
                    ..
                },
                BlockMemory::SampleTrigger { clock, initialized },
            ) => {
                *target_clock = *clock;
                *target_initialized = *initialized;
            }
            (
                Op::Edge {
                    old: target_old, ..
                },
                BlockMemory::Edge { old },
            ) => {
                *target_old = *old;
            }
            (
                Op::Timer {
                    old_active: target_old_active,
                    elapsed: target_elapsed,
                    ..
                },
                BlockMemory::Timer {
                    old_active,
                    elapsed,
                },
            ) => {
                *target_old_active = *old_active;
                *target_elapsed = *elapsed;
            }
            (_, BlockMemory::Stateless) if stateless => {}
            _ => return Err("snapshot memory family differs".into()),
        }
        Ok(())
    }
}
impl BlockMemory {
    fn valid(&self) -> bool {
        match self {
            Self::Stateless => true,
            Self::UnitDelay {
                elapsed,
                state,
                last_input,
                ..
            } => {
                elapsed.is_finite() && *elapsed >= 0. && state.is_finite() && last_input.is_finite()
            }
            Self::Pre { state } => *state == 0. || *state == 1.,
            Self::Filter { old, .. } => old.is_none_or(|v| v.is_finite()),
            Self::Rate { old, .. } => old.is_none_or(|v| v.is_finite()),
            Self::Hysteresis { .. } => true,
            Self::Pi { integral, .. } => integral.is_finite(),
            Self::PiWithReset {
                derivative_x,
                integral,
                ..
            } => derivative_x.is_none_or(|v| v.is_finite()) && integral.is_finite(),
            Self::Greater { .. } => true,
            Self::GreaterThreshold { .. } => true,
            Self::LessThreshold { .. } => true,
            Self::Less { .. } => true,
            Self::Delay { old, .. } => old.is_none_or(|v| v.is_finite()),
            Self::TrueDelay { elapsed, .. } => elapsed.is_none_or(|v| v.is_finite() && v >= 0.),
            Self::Latch { .. } => true,
            Self::TrueFalseHold { age, .. } => age.is_finite() && *age >= 0.,
            Self::TriggeredSampler { .. } => true,
            Self::Sampler { elapsed, .. } => elapsed.is_finite() && *elapsed >= 0.,
            Self::TriggeredMovingMean { samples, .. } => samples.iter().all(|v| v.is_finite()),
            Self::SampleTrigger { clock, .. } => clock.is_finite() && *clock >= 0.,
            Self::Edge { .. } => true,
            Self::Timer { elapsed, .. } => elapsed.is_finite() && *elapsed >= 0.,
        }
    }
}

// Decimal strings use Rust's correctly rounded parser, avoiding loss through JSON
// implementations that round intermediate f64 values. This also preserves -0.
#[cfg(feature = "serde")]
mod wire {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    fn parse<E: serde::de::Error>(s: String) -> Result<f64, E> {
        let v = s.parse::<f64>().map_err(E::custom)?;
        if v.is_finite() {
            Ok(v)
        } else {
            Err(E::custom("nonfinite state"))
        }
    }
    pub mod number {
        use super::*;
        pub fn serialize<S: Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
            if !v.is_finite() {
                return Err(serde::ser::Error::custom("nonfinite state"));
            }
            v.to_string().serialize(s)
        }
        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
            parse(String::deserialize(d)?)
        }
    }
    pub mod optional {
        use super::*;
        pub fn serialize<S: Serializer>(v: &Option<f64>, s: S) -> Result<S::Ok, S::Error> {
            if v.is_some_and(|v| !v.is_finite()) {
                return Err(serde::ser::Error::custom("nonfinite state"));
            }
            v.map(|v| v.to_string()).serialize(s)
        }
        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
            Option::<String>::deserialize(d)?.map(parse).transpose()
        }
    }
    pub mod queue {
        use super::*;
        use std::collections::VecDeque;
        pub fn serialize<S: Serializer>(v: &VecDeque<f64>, s: S) -> Result<S::Ok, S::Error> {
            if v.iter().any(|v| !v.is_finite()) {
                return Err(serde::ser::Error::custom("nonfinite state"));
            }
            v.iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .serialize(s)
        }
        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<VecDeque<f64>, D::Error> {
            Vec::<String>::deserialize(d)?
                .into_iter()
                .map(parse)
                .collect()
        }
    }
}
