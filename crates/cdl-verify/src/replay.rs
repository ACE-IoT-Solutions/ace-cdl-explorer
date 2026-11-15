use crate::{Result, Sample, Trace};
use cdl_exec::{PortHandle, Program, Scalar, State};
use cdl_model::{DataType, Diagnostic, DiagnosticCode, Document};
use serde_json::Value;
use std::{collections::BTreeMap, path::Path};

/// Named, numeric controller inputs at one instant; Booleans use zero and one.
#[derive(Clone, Debug)]
pub struct InputRow {
    /// Seconds since experiment start.
    pub time: f64,
    /// Values indexed by root connector name.
    pub inputs: BTreeMap<String, f64>,
}
/// Parsed CSV experiment. The optional y column is the reference output.
#[derive(Clone, Debug)]
pub struct Trend {
    /// Ordered experiment input rows.
    pub rows: Vec<InputRow>,
    /// Reference observations, if a y column was provided.
    pub reference: Option<Trace>,
}

/// Load simplified JSON or compacted CXF JSON-LD without a remote context fetch.
pub fn load_document(input: &str) -> std::result::Result<Document, Vec<Diagnostic>> {
    let value: Value = serde_json::from_str(input).map_err(|e| {
        vec![Diagnostic::new(
            "",
            DiagnosticCode::InvalidDocument,
            e.to_string(),
        )]
    })?;
    if value.get("@context").is_some() || value.get("@graph").is_some() {
        cxf::from_value(&value, &cxf::core_library())
    } else {
        cdl_compat::from_value(&value, cdl_compat::ReadOptions::default())
    }
}

/// Read RFC 4180 CSV with time_s first, named scalar inputs and optional y.
/// Times must strictly increase; Boolean strings true/false and numeric values
/// are accepted. Missing fields, duplicate headers and nonfinite values fail.
pub fn read_csv(path: impl AsRef<Path>) -> Result<Trend> {
    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_path(path)?;
    let headers = reader.headers()?.clone();
    if headers.get(0) != Some("time_s")
        || headers.iter().any(str::is_empty)
        || headers
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != headers.len()
    {
        return Err("CSV needs unique nonempty column names and time_s first".into());
    }
    let mut rows: Vec<InputRow> = Vec::new();
    let mut reference = headers.iter().any(|h| h == "y").then(Vec::new);
    for record in reader.records() {
        let record = record?;
        let parse = |raw: &str| -> Result<f64> {
            let value: f64 = match raw {
                "true" => 1.,
                "false" => 0.,
                _ => raw.parse()?,
            };
            if !value.is_finite() {
                return Err("nonfinite CSV value".into());
            }
            Ok(value)
        };
        let time = parse(&record[0])?;
        if rows.last().is_some_and(|p| p.time >= time) {
            return Err("CSV times must strictly increase".into());
        }
        let mut inputs = BTreeMap::new();
        for (name, raw) in headers.iter().zip(record.iter()).skip(1) {
            let value = parse(raw)?;
            if name == "y" {
                reference.as_mut().unwrap().push(Sample { time, value });
            }
            inputs.insert(name.into(), value);
        }
        rows.push(InputRow { time, inputs });
    }
    if rows.is_empty() {
        return Err("CSV contains no data rows".into());
    }
    Ok(Trend { rows, reference })
}

/// Host interface for a controller under test, including remote implementations.
/// Each case calls initialize once, then step at every CSV timestamp. The first
/// duration is zero, subsequent durations are differences between timestamps.
pub trait Implementation {
    /// Compile/reset a fresh experiment and select an output or trace path.
    fn initialize(&mut self, document: &Document, output: &str) -> Result<()>;
    /// Set named root inputs, advance the controller and return the selected scalar.
    fn step(&mut self, inputs: &BTreeMap<String, f64>, dt: f64) -> Result<f64>;
}

/// Adapter to the native compiled CDL executor; no building simulator is required.
#[derive(Default)]
pub struct CdlImplementation {
    state: Option<State>,
    inputs: Vec<(String, PortHandle)>,
    output: Option<PortHandle>,
}
impl Implementation for CdlImplementation {
    fn initialize(&mut self, document: &Document, output: &str) -> Result<()> {
        let program = Program::compile(
            document,
            &cdl_blocks::Registry::new(document.library.clone()),
        )?;
        let class = document
            .library
            .classes
            .get(&document.root.class)
            .ok_or("missing root class")?;
        self.inputs = class
            .inputs
            .iter()
            .map(|p| Ok((p.name.clone(), program.input(&p.name)?)))
            .collect::<Result<_>>()?;
        self.output = Some(program.output(output).or_else(|_| program.trace(output))?);
        self.state = Some(program.instantiate());
        Ok(())
    }
    fn step(&mut self, inputs: &BTreeMap<String, f64>, dt: f64) -> Result<f64> {
        let state = self
            .state
            .as_mut()
            .ok_or("implementation has not been initialized")?;
        for (name, handle) in &self.inputs {
            let &value = inputs
                .get(name)
                .ok_or_else(|| format!("missing input {name}"))?;
            let scalar =
                input_scalar(value, handle.data_type()).map_err(|e| format!("{name}: {e}"))?;
            state.set(handle, scalar)?;
        }
        state.step(dt)?;
        let value = match state.get(self.output.as_ref().unwrap())? {
            Scalar::Real(v) => v,
            Scalar::Integer(v) => v as f64,
            Scalar::Boolean(v) => {
                if v {
                    1.
                } else {
                    0.
                }
            }
        };
        if !value.is_finite() {
            return Err("controller returned a nonfinite output".into());
        }
        Ok(value)
    }
}

/// Convert a finite numeric CSV value to a declared scalar connector type.
/// Boolean inputs require zero or one; integer inputs must be integral and in range.
pub fn input_scalar(value: f64, data_type: &DataType) -> Result<Scalar> {
    match data_type {
        DataType::Real if value.is_finite() => Ok(Scalar::Real(value)),
        DataType::Boolean if value == 0. || value == 1. => Ok(Scalar::Boolean(value == 1.)),
        DataType::Integer
            if value.is_finite()
                && value.fract() == 0.
                && value >= i64::MIN as f64
                && value < (i64::MAX as f64) =>
        {
            Ok(Scalar::Integer(value as i64))
        }
        _ => Err("invalid scalar value or unsupported connector type".into()),
    }
}
