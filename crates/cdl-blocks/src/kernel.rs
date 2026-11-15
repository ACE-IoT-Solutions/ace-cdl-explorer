//! Kernels ported from the production slot executor; state belongs to each kernel.
use crate::config::PidControllerType;
use crate::slots::Slot;
use crate::{KernelError, Step, StepCtx};
use std::collections::VecDeque;
#[path = "state.rs"]
mod state;
pub use state::{BlockMemory, BlockSnapshot};
#[path = "tracking.rs"]
mod tracking;
fn clamp(x: f64, low: f64, high: f64) -> f64 {
    x.max(low).min(high)
}

#[derive(Clone)]
struct SampledDelay {
    input: Slot,
    output: Slot,
    period: f64,
    elapsed: f64,
    state: f64,
    last_input: f64,
    initialized: bool,
    sampled_now: bool,
}

#[derive(Clone)]
struct PreviousBoolean {
    input: Slot,
    output: Slot,
    state: f64,
}

#[derive(Clone)]
enum Op {
    Constant {
        out: Slot,
        value: f64,
    },
    UnitDelay(SampledDelay),
    Pre(PreviousBoolean),
    Add {
        a: Slot,
        b: Slot,
        out: Slot,
        k1: f64,
        k2: f64,
    },
    AddParameter {
        a: Slot,
        out: Slot,
        p: f64,
    },
    Min {
        a: Slot,
        b: Slot,
        out: Slot,
    },
    Max {
        a: Slot,
        b: Slot,
        out: Slot,
    },
    Line {
        x1: Slot,
        f1: Slot,
        u: Slot,
        x2: Slot,
        f2: Slot,
        out: Slot,
        limit_below: bool,
        limit_above: bool,
    },
    BooleanToInteger {
        a: Slot,
        out: Slot,
        integer_true: f64,
        integer_false: f64,
    },
    RealToInteger {
        a: Slot,
        out: Slot,
    },
    BooleanToReal {
        a: Slot,
        out: Slot,
        real_true: f64,
        real_false: f64,
    },
    IntegerToReal {
        a: Slot,
        out: Slot,
    },
    Gain {
        a: Slot,
        out: Slot,
        k: f64,
    },
    Limit {
        a: Slot,
        out: Slot,
        low: f64,
        high: f64,
    },
    Filter {
        a: Slot,
        out: Slot,
        alpha_time: f64,
        old: Option<f64>,
    },
    Rate {
        a: Slot,
        out: Slot,
        rise: f64,
        fall: f64,
        old: Option<f64>,
    },
    Hysteresis {
        a: Slot,
        out: Slot,
        low: f64,
        high: f64,
        old: bool,
    },
    Switch {
        a: Slot,
        b: Slot,
        choice: Slot,
        out: Slot,
    },
    Pi {
        setpoint: Slot,
        measured: Slot,
        out: Slot,
        reverse: bool,
        k: f64,
        ti: f64,
        low: f64,
        high: f64,
        integral: f64,
    },
    PiWithReset {
        setpoint: Slot,
        measured: Slot,
        trigger: Slot,
        out: Slot,
        reverse: bool,
        k: f64,
        ti: f64,
        scale: f64,
        low: f64,
        high: f64,
        ni: f64,
        controller_type: PidControllerType,
        td: f64,
        nd: f64,
        derivative_x: Option<f64>,
        derivative_start: f64,
        reset: f64,
        integral: f64,
        previous_trigger: bool,
        trigger_initialized: bool,
    },
    Greater {
        a: Slot,
        b: Slot,
        out: Slot,
        hysteresis: f64,
        old: bool,
    },
    GreaterThreshold {
        a: Slot,
        out: Slot,
        threshold: f64,
        hysteresis: f64,
        old: bool,
    },
    LessThreshold {
        a: Slot,
        out: Slot,
        threshold: f64,
        hysteresis: f64,
        old: bool,
    },
    Equal {
        a: Slot,
        b: Slot,
        out: Slot,
    },
    Less {
        a: Slot,
        b: Slot,
        out: Slot,
        hysteresis: f64,
        old: bool,
    },
    Subtract {
        a: Slot,
        b: Slot,
        out: Slot,
    },
    Multiply {
        a: Slot,
        b: Slot,
        out: Slot,
    },
    Divide {
        a: Slot,
        b: Slot,
        out: Slot,
    },
    And {
        a: Slot,
        b: Slot,
        out: Slot,
    },
    Or {
        a: Slot,
        b: Slot,
        out: Slot,
    },
    Not {
        a: Slot,
        out: Slot,
    },
    Delay {
        a: Slot,
        out: Slot,
        old: Option<f64>,
    },
    TrueDelay {
        a: Slot,
        out: Slot,
        delay: f64,
        delay_on_init: bool,
        elapsed: Option<f64>,
        old_active: Option<bool>,
    },
    Latch {
        set: Slot,
        clear: Slot,
        out: Slot,
        old_set: Option<bool>,
        old_clear: Option<bool>,
        state: bool,
    },
    TrueFalseHold {
        a: Slot,
        out: Slot,
        true_duration: f64,
        false_duration: f64,
        old_input: bool,
        state: bool,
        age: f64,
        initialized: bool,
    },
    TriggeredSampler {
        a: Slot,
        trigger: Slot,
        out: Slot,
        old_trigger: bool,
    },
    Sampler {
        a: Slot,
        out: Slot,
        period: f64,
        elapsed: f64,
        initialized: bool,
    },
    TriggeredMovingMean {
        a: Slot,
        trigger: Slot,
        out: Slot,
        old_trigger: bool,
        initialized: bool,
        samples: VecDeque<f64>,
        capacity: usize,
    },
    SampleTrigger {
        out: Slot,
        period: f64,
        shift: f64,
        clock: f64,
        initialized: bool,
    },
    Edge {
        a: Slot,
        out: Slot,
        old: bool,
        falling: bool,
    },
    Timer {
        a: Slot,
        out: Slot,
        passed: Slot,
        threshold: f64,
        old_active: bool,
        elapsed: f64,
    },
}

impl Op {
    fn is_stateful(&self) -> bool {
        // Preserve the production executor's event-checkpoint set, including
        // its omission of Greater/Less. Changing that set can alter hysteresis
        // during Pre iteration and requires a separate behavioral regression.
        matches!(
            self,
            Self::UnitDelay(_)
                | Self::Pre(_)
                | Self::Filter { .. }
                | Self::Rate { .. }
                | Self::Hysteresis { .. }
                | Self::Pi { .. }
                | Self::PiWithReset { .. }
                | Self::GreaterThreshold { .. }
                | Self::LessThreshold { .. }
                | Self::Delay { .. }
                | Self::TrueDelay { .. }
                | Self::Latch { .. }
                | Self::TrueFalseHold { .. }
                | Self::TriggeredSampler { .. }
                | Self::Sampler { .. }
                | Self::TriggeredMovingMean { .. }
                | Self::SampleTrigger { .. }
                | Self::Edge { .. }
                | Self::Timer { .. }
        )
    }
}

/// An owned built-in step. Cloning creates an independent state/checkpoint.
#[derive(Clone)]
pub struct Kernel {
    op: Op,
    initial: Option<(Slot, f64)>,
}
impl Kernel {
    /// Whether initialization permanently determines this kernel's output.
    /// Compilers may omit such kernels from the stepping loop after initialization.
    pub fn is_constant(&self) -> bool {
        matches!(self.op, Op::Constant { .. })
    }
}
impl Step for Kernel {
    fn initialize(&self, slots: &mut [f64]) {
        if let Some((slot, value)) = &self.initial {
            slots[*slot] = *value;
        }
    }
    fn is_stateful(&self) -> bool {
        self.op.is_stateful()
    }
    fn prepare(&mut self, slots: &mut [f64], ctx: &StepCtx) {
        match &mut self.op {
            Op::UnitDelay(delay) => {
                if delay.initialized {
                    delay.elapsed += ctx.dt();
                }
                delay.sampled_now = delay.initialized && delay.elapsed + 1e-9 >= delay.period;
                if delay.sampled_now {
                    delay.elapsed %= delay.period;
                    delay.state = delay.last_input;
                }
                slots[delay.output] = delay.state;
            }
            Op::Pre(previous) => slots[previous.output] = previous.state,
            _ => {}
        }
    }
    #[inline(always)]
    fn step(&mut self, slots: &mut [f64], ctx: &StepCtx) -> Result<(), KernelError> {
        let dt = ctx.dt();
        let op = &mut self.op;
        let s = slots;
        match op {
            Op::Constant { out, value } => s[*out] = *value,
            Op::UnitDelay(_) | Op::Pre(_) => {}
            Op::Add { a, b, out, k1, k2 } => s[*out] = *k1 * s[*a] + *k2 * s[*b],
            Op::AddParameter { a, out, p } => s[*out] = s[*a] + *p,
            Op::Min { a, b, out } => s[*out] = s[*a].min(s[*b]),
            Op::Max { a, b, out } => s[*out] = s[*a].max(s[*b]),
            Op::Line {
                x1,
                f1,
                u,
                x2,
                f2,
                out,
                limit_below,
                limit_above,
            } => {
                if s[*x2] <= s[*x1] {
                    return Err("Line requires x2 > x1".into());
                }
                let mut x = s[*u];
                if *limit_below {
                    x = x.max(s[*x1]);
                }
                if *limit_above {
                    x = x.min(s[*x2]);
                }
                s[*out] = s[*f1] + (s[*f2] - s[*f1]) * (x - s[*x1]) / (s[*x2] - s[*x1]);
            }
            Op::BooleanToInteger {
                a,
                out,
                integer_true,
                integer_false,
            } => {
                s[*out] = if s[*a] != 0.0 {
                    *integer_true
                } else {
                    *integer_false
                }
            }
            Op::RealToInteger { a, out } => s[*out] = s[*a].round(),
            Op::BooleanToReal {
                a,
                out,
                real_true,
                real_false,
            } => {
                s[*out] = if s[*a] != 0.0 {
                    *real_true
                } else {
                    *real_false
                }
            }
            Op::IntegerToReal { a, out } => s[*out] = s[*a],
            Op::Gain { a, out, k } => s[*out] = *k * s[*a],
            Op::Limit { a, out, low, high } => s[*out] = clamp(s[*a], *low, *high),
            Op::Filter {
                a,
                out,
                alpha_time,
                old,
            } => {
                let previous = old.unwrap_or(s[*a]);
                let value = previous + (1.0 - (-dt / *alpha_time).exp()) * (s[*a] - previous);
                *old = Some(value);
                s[*out] = value;
            }
            Op::Rate {
                a,
                out,
                rise,
                fall,
                old,
            } => {
                let previous = old.unwrap_or(s[*a]);
                let value = clamp(s[*a], previous - *fall * dt, previous + *rise * dt);
                *old = Some(value);
                s[*out] = value;
            }
            Op::Hysteresis {
                a,
                out,
                low,
                high,
                old,
            } => {
                if s[*a] > *high {
                    *old = true
                } else if s[*a] < *low {
                    *old = false
                };
                s[*out] = if *old { 1.0 } else { 0.0 };
            }
            Op::Switch { a, b, choice, out } => {
                s[*out] = if s[*choice] != 0.0 { s[*a] } else { s[*b] }
            }
            Op::Pi {
                setpoint,
                measured,
                out,
                reverse,
                k,
                ti,
                low,
                high,
                integral,
            } => {
                let e = (s[*setpoint] - s[*measured]) * if *reverse { 1.0 } else { -1.0 };
                let proposal = *integral + *k * e * dt / *ti;
                *integral = clamp(proposal, *low - *k * e, *high - *k * e);
                s[*out] = clamp(*k * e + *integral, *low, *high);
            }
            Op::PiWithReset {
                setpoint,
                measured,
                trigger,
                out,
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
                derivative_x,
                derivative_start,
                reset,
                integral,
                previous_trigger,
                trigger_initialized,
            } => {
                let e = (s[*setpoint] - s[*measured]) * if *reverse { 1.0 } else { -1.0 } / *scale;
                let proportional = *k * e;
                let derivative = if controller_type.with_derivative() {
                    let filter_time = *td / *nd;
                    let derivative_gain = *k * *td;
                    let old_x = derivative_x
                        .get_or_insert(e - filter_time * *derivative_start / derivative_gain);
                    *old_x = e + (*old_x - e) * (-dt / filter_time).exp();
                    derivative_gain / filter_time * (e - *old_x)
                } else {
                    0.0
                };
                let feedforward = proportional + derivative;
                let triggered = s[*trigger] != 0.0;
                if !*trigger_initialized {
                    *previous_trigger = triggered;
                    *trigger_initialized = true;
                }
                if controller_type.with_integral() {
                    if triggered && !*previous_trigger {
                        *integral = *reset - feedforward;
                    } else {
                        let unsaturated = feedforward + *integral;
                        let saturated = clamp(unsaturated, *low, *high);
                        let antiwindup = (unsaturated - saturated) / (*k * *ni).max(1e-12);
                        *integral += *k * dt / *ti * (e - antiwindup);
                    }
                }
                *previous_trigger = triggered;
                s[*out] = clamp(
                    feedforward
                        + if controller_type.with_integral() {
                            *integral
                        } else {
                            0.0
                        },
                    *low,
                    *high,
                );
            }
            Op::Greater {
                a,
                b,
                out,
                hysteresis,
                old,
            } => {
                *old = s[*a]
                    > s[*b]
                        - if *hysteresis >= 1e-10 && *old {
                            *hysteresis
                        } else {
                            0.0
                        };
                s[*out] = if *old { 1.0 } else { 0.0 };
            }
            Op::GreaterThreshold {
                a,
                out,
                threshold,
                hysteresis,
                old,
            } => {
                *old = if *hysteresis >= 1e-10 {
                    if *old {
                        s[*a] > *threshold - *hysteresis
                    } else {
                        s[*a] > *threshold
                    }
                } else {
                    s[*a] > *threshold
                };
                s[*out] = if *old { 1.0 } else { 0.0 };
            }
            Op::LessThreshold {
                a,
                out,
                threshold,
                hysteresis,
                old,
            } => {
                *old = if *hysteresis >= 1e-10 {
                    if *old {
                        s[*a] < *threshold + *hysteresis
                    } else {
                        s[*a] < *threshold
                    }
                } else {
                    s[*a] < *threshold
                };
                s[*out] = if *old { 1.0 } else { 0.0 };
            }
            Op::Equal { a, b, out } => s[*out] = if s[*a] == s[*b] { 1.0 } else { 0.0 },
            Op::Less {
                a,
                b,
                out,
                hysteresis,
                old,
            } => {
                *old = s[*a]
                    < s[*b]
                        + if *hysteresis >= 1e-10 && *old {
                            *hysteresis
                        } else {
                            0.0
                        };
                s[*out] = if *old { 1.0 } else { 0.0 };
            }
            Op::Subtract { a, b, out } => s[*out] = s[*a] - s[*b],
            Op::Multiply { a, b, out } => s[*out] = s[*a] * s[*b],
            Op::Divide { a, b, out } => s[*out] = s[*a] / s[*b],
            Op::And { a, b, out } => {
                s[*out] = if s[*a] != 0.0 && s[*b] != 0.0 {
                    1.0
                } else {
                    0.0
                }
            }
            Op::Or { a, b, out } => {
                s[*out] = if s[*a] != 0.0 || s[*b] != 0.0 {
                    1.0
                } else {
                    0.0
                }
            }
            Op::Not { a, out } => s[*out] = if s[*a] == 0.0 { 1.0 } else { 0.0 },
            Op::Delay { a, out, old } => {
                let previous = old.unwrap_or(s[*a]);
                *old = Some(s[*a]);
                s[*out] = previous;
            }
            Op::TrueDelay {
                a,
                out,
                delay,
                delay_on_init,
                elapsed,
                old_active,
            } => {
                let active = s[*a] != 0.0;
                let next = if active {
                    if old_active.is_none() && !*delay_on_init {
                        *delay
                    } else if *old_active == Some(true) {
                        elapsed.unwrap_or(0.0) + dt
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };
                *elapsed = Some(next);
                *old_active = Some(active);
                s[*out] = if active && next >= *delay { 1.0 } else { 0.0 };
            }
            Op::Latch {
                set,
                clear,
                out,
                old_set,
                old_clear,
                state,
            } => {
                let set_now = s[*set] != 0.0;
                let clear_now = s[*clear] != 0.0;
                if let (Some(before_set), Some(before_clear)) = (*old_set, *old_clear) {
                    if (!before_set && set_now) || (!before_clear && clear_now) {
                        *state = set_now && !clear_now;
                    }
                } else {
                    *state = set_now && !clear_now;
                }
                *old_set = Some(set_now);
                *old_clear = Some(clear_now);
                s[*out] = if *state { 1.0 } else { 0.0 };
            }
            Op::TrueFalseHold {
                a,
                out,
                true_duration,
                false_duration,
                old_input,
                state,
                age,
                initialized,
            } => {
                let active = s[*a] != 0.0;
                if *initialized {
                    let previous_age = *age;
                    *age += dt;
                    let minimum = if *state {
                        *true_duration
                    } else {
                        *false_duration
                    };
                    if active != *state && *age >= minimum {
                        *age = if active != *old_input && previous_age >= minimum {
                            0.0
                        } else {
                            *age - minimum
                        };
                        *state = active;
                    }
                } else {
                    *state = active;
                    *initialized = true;
                }
                *old_input = active;
                s[*out] = if *state { 1.0 } else { 0.0 };
            }
            Op::TriggeredSampler {
                a,
                trigger,
                out,
                old_trigger,
            } => {
                let active = s[*trigger] != 0.0;
                if active && !*old_trigger {
                    s[*out] = s[*a];
                }
                *old_trigger = active;
            }
            Op::Sampler {
                a,
                out,
                period,
                elapsed,
                initialized,
            } => {
                if *initialized {
                    *elapsed += dt;
                }
                let event = *initialized && *elapsed + 1e-9 >= *period;
                if event {
                    *elapsed %= *period;
                }
                if event || !*initialized {
                    s[*out] = s[*a];
                    *initialized = true;
                }
            }
            Op::TriggeredMovingMean {
                a,
                trigger,
                out,
                old_trigger,
                initialized,
                samples,
                capacity,
            } => {
                let active = s[*trigger] != 0.0;
                if !*initialized || (active && !*old_trigger) {
                    samples.push_back(s[*a]);
                    if samples.len() > *capacity {
                        samples.pop_front();
                    }
                    s[*out] = samples.iter().sum::<f64>() / samples.len() as f64;
                }
                *initialized = true;
                *old_trigger = active;
            }
            Op::SampleTrigger {
                out,
                period,
                shift,
                clock,
                initialized,
            } => {
                let before = *clock;
                if *initialized {
                    *clock += dt;
                }
                let crossed = *initialized
                    && ((before - *shift + 1e-9) / *period).floor()
                        < ((*clock - *shift + 1e-9) / *period).floor();
                s[*out] = if (!*initialized && *shift <= 1e-9) || crossed {
                    1.0
                } else {
                    0.0
                };
                *initialized = true;
            }
            Op::Edge {
                a,
                out,
                old,
                falling,
            } => {
                let active = s[*a] != 0.0;
                s[*out] = if if *falling {
                    !active && *old
                } else {
                    active && !*old
                } {
                    1.0
                } else {
                    0.0
                };
                *old = active;
            }
            Op::Timer {
                a,
                out,
                passed,
                threshold,
                old_active,
                elapsed,
            } => {
                let active = s[*a] != 0.0;
                *elapsed = if !active || !*old_active {
                    0.0
                } else {
                    *elapsed + dt
                };
                s[*out] = *elapsed;
                s[*passed] = if active && *elapsed >= *threshold {
                    1.0
                } else {
                    0.0
                };
                *old_active = active;
            }
        }
        Ok(())
    }
    fn commit(&mut self, slots: &[f64]) {
        match &mut self.op {
            Op::UnitDelay(delay) => {
                if delay.sampled_now || !delay.initialized {
                    delay.last_input = slots[delay.input];
                    delay.initialized = true;
                }
            }
            Op::Pre(previous) => {
                previous.state = if slots[previous.input] != 0.0 {
                    1.0
                } else {
                    0.0
                }
            }
            _ => {}
        }
    }
}
impl Kernel {
    /// Current Boolean Pre state, if this is an event-loop breaker.
    pub fn event_state(&self) -> Option<f64> {
        match &self.op {
            Op::Pre(p) => Some(p.state),
            _ => None,
        }
    }
    /// Supply an event-iteration guess after restoring a checkpoint.
    pub fn set_event_state(&mut self, value: f64) {
        if let Op::Pre(p) = &mut self.op {
            p.state = value;
        }
    }
}

#[path = "factory.rs"]
mod factory;
pub(crate) use factory::build;
