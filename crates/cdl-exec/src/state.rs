use crate::instruction::Instruction;
use crate::{Data, Error, PortHandle, Scalar};
use cdl_blocks::{Step, StepCtx};
use cdl_model::DataType;
use std::sync::Arc;

/// Independently owned mutable state of a compiled program.
#[derive(Clone)]
pub struct State {
    pub(crate) data: Arc<Data>,
    pub(crate) slots: Vec<f64>,
    pub(crate) kernels: Vec<Instruction>,
    pub(crate) inputs_set: Vec<bool>,
}
impl State {
    pub(crate) fn new(data: Arc<Data>) -> Self {
        Self {
            slots: data.slots.clone(),
            kernels: data.kernels.clone(),
            inputs_set: vec![false; data.inputs.len()],
            data,
        }
    }
    /// Assign a root input by name. Resolve a handle for repeated writes.
    pub fn set_input(&mut self, name: &str, value: Scalar) -> Result<(), Error> {
        let h = self
            .data
            .inputs
            .get(name)
            .cloned()
            .ok_or_else(|| Error::UnknownPort(name.into()))?;
        self.set(&h, value)
    }
    /// Assign a previously resolved root input with no string lookup.
    pub fn set(&mut self, handle: &PortHandle, value: Scalar) -> Result<(), Error> {
        if handle.program != self.data.id || handle.input.is_none() {
            return Err(Error::InvalidHandle);
        }
        let value = match (value, &handle.data_type) {
            (Scalar::Real(v), DataType::Real) if v.is_finite() => v,
            (Scalar::Real(_), DataType::Real) => return Err(Error::InvalidValue),
            (Scalar::Integer(v), DataType::Integer | DataType::Real) => v as f64,
            (Scalar::Boolean(v), DataType::Boolean) => {
                if v {
                    1.0
                } else {
                    0.0
                }
            }
            _ => return Err(Error::TypeMismatch),
        };
        self.slots[handle.slot] = value;
        self.inputs_set[handle.input.unwrap()] = true;
        Ok(())
    }
    /// Read a root output by name.
    pub fn output(&self, name: &str) -> Result<Scalar, Error> {
        let h = self
            .data
            .outputs
            .get(name)
            .ok_or_else(|| Error::UnknownPort(name.into()))?;
        self.get(h)
    }
    /// Read a previously resolved output or internal connector.
    pub fn get(&self, handle: &PortHandle) -> Result<Scalar, Error> {
        if handle.program != self.data.id {
            return Err(Error::InvalidHandle);
        }
        let value = self.slots[handle.slot];
        Ok(match handle.data_type {
            DataType::Boolean => Scalar::Boolean(value != 0.0),
            DataType::Integer => Scalar::Integer(value as i64),
            _ => Scalar::Real(value),
        })
    }
    /// Advance the program, including simultaneous delay capture and Pre iteration.
    pub fn step(&mut self, dt_seconds: f64) -> Result<(), Error> {
        let ctx = StepCtx::new(dt_seconds).map_err(|_| Error::InvalidStep)?;
        if self.inputs_set.iter().any(|set| !set) {
            let name = self
                .data
                .inputs
                .iter()
                .find(|(_, h)| !self.inputs_set[h.input.unwrap()])
                .unwrap()
                .0;
            return Err(Error::MissingInput(name.clone()));
        }
        if self.data.previous.is_empty() {
            return self.once(&ctx);
        }
        let slots = self.slots.clone();
        let checkpoint: Vec<_> = self
            .data
            .stateful
            .iter()
            .map(|&i| (i, self.kernels[i].clone()))
            .collect();
        let mut guess: Vec<_> = self
            .data
            .previous
            .iter()
            .map(|&i| self.kernels[i].event_state().unwrap())
            .collect();
        for _ in 0..32 {
            self.restore_event_checkpoint(&slots, &checkpoint);
            for (&index, &state) in self.data.previous.iter().zip(&guess) {
                self.kernels[index].set_event_state(state);
            }
            if let Err(error) = self.once(&ctx) {
                self.restore_event_checkpoint(&slots, &checkpoint);
                return Err(error);
            }
            let next: Vec<_> = self
                .data
                .previous
                .iter()
                .map(|&i| self.kernels[i].event_state().unwrap())
                .collect();
            if next == guess {
                return Ok(());
            }
            guess = next;
        }
        self.restore_event_checkpoint(&slots, &checkpoint);
        Err(Error::NonConvergent)
    }
    /// Track a PI/PID instance to an actuator output using the currently assigned inputs.
    /// This initializes memory for a handover; it does not relax checkpoint compatibility.
    pub fn track_output(&mut self, path: &str, target: f64) -> Result<(), Error> {
        if self.inputs_set.iter().any(|set| !set) {
            return Err(Error::MissingInput(
                "assign all root inputs before tracking".into(),
            ));
        }
        let index = self
            .data
            .paths
            .iter()
            .position(|p| p == path)
            .ok_or_else(|| Error::UnknownPort(path.into()))?;
        match &mut self.kernels[index] {
            Instruction::Builtin(kernel) => {
                kernel
                    .track_output(&mut self.slots, target)
                    .map_err(|e| Error::Kernel {
                        path: path.into(),
                        message: e.to_string(),
                    })
            }
            #[cfg(feature = "equations")]
            Instruction::Equations(_) => Err(Error::Kernel {
                path: path.into(),
                message: "equation blocks do not support tracking".into(),
            }),
        }
    }
    fn restore_event_checkpoint(&mut self, slots: &[f64], kernels: &[(usize, Instruction)]) {
        self.slots.clone_from_slice(slots);
        for (index, kernel) in kernels {
            self.kernels[*index].clone_from(kernel);
        }
    }
    fn once(&mut self, ctx: &StepCtx) -> Result<(), Error> {
        for &index in &self.data.breakers {
            self.kernels[index].prepare(&mut self.slots, ctx);
        }
        for (index, kernel) in self.kernels.iter_mut().enumerate() {
            kernel
                .step(&mut self.slots, ctx)
                .map_err(|e| Error::Kernel {
                    path: self.data.paths[index].clone(),
                    message: e.to_string(),
                })?;
        }
        for &index in &self.data.breakers {
            self.kernels[index].commit(&self.slots);
        }
        Ok(())
    }
}
