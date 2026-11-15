//! Portable checkpoints keyed by semantic paths, independent of slot numbering.
use crate::{instruction::Instruction, State};
use cdl_blocks::BlockSnapshot;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A complete execution checkpoint, exported only between completed steps.
///
/// Recompile the same document, instantiate a state, then restore. Compatibility
/// includes the entire document, including configuration/annotations, plus block
/// configuration identities. Signals use canonical paths; alias agreement is
/// checked. Runtime program IDs and numeric slot addresses are never serialized.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateSnapshot {
    /// Checkpoint schema version, currently one.
    pub version: u32,
    /// SHA-256 of the serialized canonical document.
    pub program_fingerprint: [u8; 32],
    /// Canonical connector values, including held outputs and constant slots.
    #[serde(with = "signals")]
    pub signals: BTreeMap<String, f64>,
    /// Memory for every executable built-in instance, including comparator state.
    pub blocks: BTreeMap<String, BlockSnapshot>,
    /// Root input names and whether a value has been assigned.
    pub inputs_initialized: BTreeMap<String, bool>,
}
/// A malformed or incompatible checkpoint; restoration leaves the state unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotError {
    /// Validation failure explanation, including a signal/block path when applicable.
    pub message: String,
}
impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid checkpoint: {}", self.message)
    }
}
impl std::error::Error for SnapshotError {}
fn invalid(message: impl Into<String>) -> SnapshotError {
    SnapshotError {
        message: message.into(),
    }
}
impl State {
    /// Export state without serializing the immutable program or compiler bindings.
    pub fn snapshot(&self) -> Result<StateSnapshot, SnapshotError> {
        if self.slots.iter().any(|v| !v.is_finite()) {
            return Err(invalid("nonfinite signal"));
        }
        let signals = self
            .data
            .signal_paths
            .iter()
            .map(|(p, &i)| (p.clone(), self.slots[i]))
            .collect();
        let mut blocks = BTreeMap::new();
        for (path, kernel) in self.data.paths.iter().zip(&self.kernels) {
            match kernel {
                Instruction::Builtin(k) => {
                    let snapshot = k.snapshot();
                    snapshot
                        .validate()
                        .map_err(|e| invalid(format!("{path}: {e}")))?;
                    blocks.insert(path.clone(), snapshot);
                }
                #[cfg(feature = "equations")]
                Instruction::Equations(_) => {}
            }
        }
        let inputs_initialized = self
            .data
            .inputs
            .iter()
            .map(|(name, h)| (name.clone(), self.inputs_set[h.input.unwrap()]))
            .collect();
        Ok(StateSnapshot {
            version: 1,
            program_fingerprint: self.data.fingerprint,
            signals,
            blocks,
            inputs_initialized,
        })
    }
    /// Validate then atomically restore a checkpoint into this program's state.
    /// Existing handles remain valid. Failure leaves the entire state unchanged.
    pub fn restore(&mut self, snapshot: &StateSnapshot) -> Result<(), SnapshotError> {
        if snapshot.version != 1 || snapshot.program_fingerprint != self.data.fingerprint {
            return Err(invalid("schema or program configuration differs"));
        }
        if snapshot.signals.len() != self.data.signal_paths.len()
            || snapshot.inputs_initialized.len() != self.data.inputs.len()
        {
            return Err(invalid("signal or input set differs"));
        }
        let mut slots = self.slots.clone();
        let mut seen = BTreeMap::<usize, u64>::new();
        for (path, &slot) in &self.data.signal_paths {
            let value = *snapshot
                .signals
                .get(path)
                .ok_or_else(|| invalid(format!("missing signal {path}")))?;
            if !value.is_finite() {
                return Err(invalid("nonfinite signal"));
            }
            if seen
                .insert(slot, value.to_bits())
                .is_some_and(|old| old != value.to_bits())
            {
                return Err(invalid("aliased connector values disagree"));
            }
            slots[slot] = value;
        }
        for &slot in &self.data.constant_slots {
            if slots[slot].to_bits() != self.data.slots[slot].to_bits() {
                return Err(invalid(
                    "constant connector value differs from program configuration",
                ));
            }
        }
        let mut inputs_set = self.inputs_set.clone();
        for (name, handle) in &self.data.inputs {
            inputs_set[handle.input.unwrap()] = *snapshot
                .inputs_initialized
                .get(name)
                .ok_or_else(|| invalid("missing input initialization flag"))?;
        }
        let mut kernels = self.kernels.clone();
        let mut count = 0;
        for (path, kernel) in self.data.paths.iter().zip(&mut kernels) {
            match kernel {
                Instruction::Builtin(k) => {
                    let block = snapshot
                        .blocks
                        .get(path)
                        .ok_or_else(|| invalid(format!("missing block {path}")))?;
                    k.restore(block)
                        .map_err(|e| invalid(format!("{path}: {e}")))?;
                    count += 1;
                }
                #[cfg(feature = "equations")]
                Instruction::Equations(_) => {}
            }
        }
        if count != snapshot.blocks.len() {
            return Err(invalid("unexpected block memory"));
        }
        self.slots = slots;
        self.kernels = kernels;
        self.inputs_set = inputs_set;
        Ok(())
    }
}

mod signals {
    use super::*;
    pub fn serialize<S: serde::Serializer>(
        values: &BTreeMap<String, f64>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        values
            .iter()
            .map(|(name, value)| (name, value.to_string()))
            .collect::<BTreeMap<_, _>>()
            .serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> Result<BTreeMap<String, f64>, D::Error> {
        BTreeMap::<String, String>::deserialize(d)?
            .into_iter()
            .map(|(name, value)| {
                let parsed = value.parse::<f64>().map_err(serde::de::Error::custom)?;
                if !parsed.is_finite() {
                    return Err(serde::de::Error::custom("nonfinite signal"));
                }
                Ok((name, parsed))
            })
            .collect()
    }
}

pub(crate) fn fingerprint(document: &cdl_model::Document) -> Result<[u8; 32], crate::Error> {
    use sha2::Digest;
    fn sorted(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                let mut entries = std::mem::take(map).into_iter().collect::<Vec<_>>();
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                for (key, mut value) in entries {
                    sorted(&mut value);
                    map.insert(key, value);
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    sorted(value);
                }
            }
            _ => {}
        }
    }
    let error = |e: serde_json::Error| {
        crate::Error::Diagnostics(vec![cdl_model::Diagnostic::new(
            "",
            cdl_model::DiagnosticCode::InvalidDocument,
            e.to_string(),
        )])
    };
    let mut value = serde_json::to_value(document).map_err(error)?;
    sorted(&mut value);
    Ok(sha2::Sha256::digest(serde_json::to_vec(&value).map_err(error)?).into())
}
