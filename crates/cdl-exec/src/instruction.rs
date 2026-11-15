use cdl_blocks::{Kernel, KernelError, Step, StepCtx};

#[derive(Clone)]
pub(crate) enum Instruction {
    Builtin(Kernel),
    #[cfg(feature = "equations")]
    Equations(crate::equations::Equations),
}
impl Instruction {
    pub(crate) fn is_constant(&self) -> bool {
        match self {
            Self::Builtin(k) => k.is_constant(),
            #[cfg(feature = "equations")]
            Self::Equations(_) => false,
        }
    }
    pub(crate) fn event_state(&self) -> Option<f64> {
        match self {
            Self::Builtin(k) => k.event_state(),
            #[cfg(feature = "equations")]
            Self::Equations(_) => None,
        }
    }
    pub(crate) fn set_event_state(&mut self, state: f64) {
        match self {
            Self::Builtin(k) => k.set_event_state(state),
            #[cfg(feature = "equations")]
            Self::Equations(_) => {}
        }
    }
}
impl Step for Instruction {
    fn initialize(&self, slots: &mut [f64]) {
        match self {
            Self::Builtin(k) => k.initialize(slots),
            #[cfg(feature = "equations")]
            Self::Equations(_) => {}
        }
    }
    fn is_stateful(&self) -> bool {
        match self {
            Self::Builtin(k) => k.is_stateful(),
            #[cfg(feature = "equations")]
            Self::Equations(_) => false,
        }
    }
    fn prepare(&mut self, slots: &mut [f64], ctx: &StepCtx) {
        match self {
            Self::Builtin(k) => k.prepare(slots, ctx),
            #[cfg(feature = "equations")]
            Self::Equations(_) => {}
        }
    }
    #[inline(always)]
    fn step(&mut self, slots: &mut [f64], ctx: &StepCtx) -> Result<(), KernelError> {
        match self {
            Self::Builtin(k) => k.step(slots, ctx),
            #[cfg(feature = "equations")]
            Self::Equations(e) => e.step(slots),
        }
    }
    fn commit(&mut self, slots: &[f64]) {
        match self {
            Self::Builtin(k) => k.commit(slots),
            #[cfg(feature = "equations")]
            Self::Equations(_) => {}
        }
    }
}
