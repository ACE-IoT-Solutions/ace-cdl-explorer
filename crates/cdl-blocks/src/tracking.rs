//! Explicit PI/PID tracking for controller handover.
use super::{clamp, Kernel, Op};
use crate::KernelError;
impl Kernel {
    /// Initialize PI/PID memory to the desired output at the supplied input values.
    ///
    /// This is a separate operation from exact checkpoint restoration. The target
    /// must lie inside output limits. Derivative memory is initialized at the
    /// current error (zero derivative contribution); an already-high reset input
    /// is acknowledged without inventing a rising edge. A zero-duration step at
    /// unchanged inputs preserves the target. Subsequent nonzero steps integrate
    /// normally. P/PD and other families return an error because they cannot track
    /// an arbitrary output without an integral/bias state. Errors do not mutate.
    pub fn track_output(&mut self, slots: &mut [f64], target: f64) -> Result<(), KernelError> {
        if !target.is_finite() {
            return Err("tracking target must be finite".into());
        }
        let read = |slot: usize| {
            slots
                .get(slot)
                .copied()
                .filter(|v| v.is_finite())
                .ok_or_else(|| KernelError("tracking requires finite bound inputs".into()))
        };
        match &mut self.op {
            Op::Pi {
                setpoint,
                measured,
                out,
                reverse,
                k,
                low,
                high,
                integral,
                ..
            } => {
                let e = (read(*setpoint)? - read(*measured)?) * if *reverse { 1. } else { -1. };
                let tracked = target - *k * e;
                if !tracked.is_finite()
                    || ((*k * e + tracked) - target).abs() > 1e-12 * target.abs().max(1.)
                    || target < *low
                    || target > *high
                    || *out >= slots.len()
                {
                    return Err("tracking target or bound output is invalid".into());
                }
                *integral = tracked;
                slots[*out] = target;
            }
            Op::PiWithReset {
                setpoint,
                measured,
                trigger,
                out,
                reverse,
                k,
                scale,
                low,
                high,
                controller_type,
                derivative_x,
                integral,
                previous_trigger,
                trigger_initialized,
                ..
            } => {
                if !controller_type.with_integral() {
                    return Err("tracking requires PI or PID mode".into());
                }
                let e = (read(*setpoint)? - read(*measured)?) * if *reverse { 1. } else { -1. }
                    / *scale;
                let triggered = read(*trigger)? != 0.;
                let tracked = target - *k * e;
                if !tracked.is_finite()
                    || ((*k * e + tracked) - target).abs() > 1e-12 * target.abs().max(1.)
                    || !e.is_finite()
                    || target < *low
                    || target > *high
                    || *out >= slots.len()
                {
                    return Err("tracking target or bound output is invalid".into());
                }
                *integral = tracked;
                *derivative_x = Some(e);
                *previous_trigger = triggered;
                *trigger_initialized = true;
                slots[*out] = clamp(target, *low, *high);
            }
            _ => return Err("output tracking is supported only for PI/PID kernels".into()),
        }
        Ok(())
    }
}
