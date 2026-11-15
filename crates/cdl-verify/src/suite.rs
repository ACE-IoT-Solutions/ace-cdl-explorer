use crate::{compare, load_document, read_csv, Comparison, Implementation, Result, Sample, Trace};
use cdl_model::Document;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

/// Version two verification manifest. All relative paths resolve beside the suite.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Suite {
    /// Must equal two.
    pub version: u8,
    /// Upstream reference source revision, retained in reports.
    #[serde(default)]
    pub reference_commit: String,
    /// SHA-256 digests of source/reference artifacts, keyed by relative filename.
    pub reference_files: BTreeMap<String, String>,
    /// Ordered experiments.
    pub cases: Vec<Case>,
    #[serde(skip)]
    directory: PathBuf,
}
/// One reproducible input experiment and selected output.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Unique filesystem-safe experiment identifier.
    pub id: String,
    /// Human-readable provenance.
    #[serde(default)]
    pub source: String,
    /// Inline simplified graph or CXF JSON-LD document, exclusive with graph_file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<Value>,
    /// Graph document path, relative to the suite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph_file: Option<String>,
    /// Child instance names selecting a nested controller as the experiment root.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub graph_path: Vec<String>,
    /// Input/reference CSV path relative to the suite.
    pub trend: String,
    /// Root output name (default y), or executor trace path.
    #[serde(default = "default_output")]
    pub output: String,
    /// Constant offsets applied to Real inputs, before replay.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub input_offsets: BTreeMap<String, f64>,
    /// Absolute horizontal tolerance in seconds.
    pub time_tolerance_s: f64,
    /// Absolute vertical tolerance in output units.
    pub value_tolerance: f64,
}
fn default_output() -> String {
    "y".into()
}
/// Replay result, retaining the actual trace for independent comparisons.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaseResult {
    /// Experiment identifier.
    pub id: String,
    /// Actual controller observations.
    pub actual: Trace,
    /// Native funnel report.
    pub comparison: Comparison,
}
impl Suite {
    /// Load a version-two manifest and verify its pinned files before any execution.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let mut suite: Self = serde_json::from_str(&fs::read_to_string(path)?)?;
        suite.directory = path.parent().unwrap_or(Path::new(".")).to_owned();
        if suite.version != 2 {
            return Err("unsupported suite version; expected 2".into());
        }
        if suite.cases.is_empty() {
            return Err("suite contains no cases".into());
        }
        let mut ids = HashSet::new();
        for c in &suite.cases {
            if c.id.is_empty()
                || !c
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                || !ids.insert(&c.id)
            {
                return Err("case IDs must be unique and filesystem-safe".into());
            }
            if c.graph.is_some() == c.graph_file.is_some() {
                return Err(format!("{}: specify exactly one graph or graph_file", c.id).into());
            }
            if [c.time_tolerance_s, c.value_tolerance]
                .iter()
                .any(|v| !v.is_finite() || *v < 0.)
            {
                return Err(format!("{}: tolerances must be finite and nonnegative", c.id).into());
            }
        }
        suite.verify_pins()?;
        Ok(suite)
    }
    /// Recheck every pinned SHA-256 digest, including any source files in the manifest.
    pub fn verify_pins(&self) -> Result<()> {
        for (name, expected) in &self.reference_files {
            if expected.len() != 64 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(format!("invalid SHA-256 for {name}").into());
            }
            let actual = format!("{:x}", Sha256::digest(fs::read(self.directory.join(name))?));
            if !actual.eq_ignore_ascii_case(expected) {
                return Err(format!("pinned reference changed: {name}").into());
            }
        }
        for c in &self.cases {
            if !self.reference_files.contains_key(&c.trend) {
                return Err(format!("{}: trend has no reference_files pin", c.id).into());
            }
        }
        Ok(())
    }
    /// Load a case's controller and select any nested instance.
    pub fn document(&self, case: &Case) -> Result<Document> {
        let value = match (&case.graph, &case.graph_file) {
            (Some(v), None) => v.clone(),
            (None, Some(path)) => {
                serde_json::from_str(&fs::read_to_string(self.directory.join(path))?)?
            }
            _ => return Err("specify exactly one graph or graph_file".into()),
        };
        // Select simplified subgraphs before conversion: this preserves the legacy
        // interface/default boundary used by the independent pre-workspace runner.
        let is_cxf = value.get("@context").is_some() || value.get("@graph").is_some();
        let mut selected = &value;
        if !is_cxf {
            for name in &case.graph_path {
                selected = selected
                    .get("blocks")
                    .and_then(Value::as_array)
                    .and_then(|blocks| {
                        blocks
                            .iter()
                            .find(|v| v.get("name").and_then(Value::as_str) == Some(name))
                    })
                    .ok_or_else(|| format!("missing graph_path child {name}"))?;
            }
        }
        let mut document = load_document(&serde_json::to_string(selected)?).map_err(|d| {
            d.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        })?;
        if is_cxf {
            for name in &case.graph_path {
                document.root = document
                    .library
                    .classes
                    .get(&document.root.class)
                    .and_then(|c| c.body.as_ref())
                    .and_then(|b| b.instances.iter().find(|i| i.name == *name))
                    .cloned()
                    .ok_or_else(|| format!("missing graph_path child {name}"))?;
            }
        }
        Ok(document)
    }
    /// Replay one case with a fresh implementation, then compare its selected output.
    pub fn run_case(
        &self,
        case: &Case,
        implementation: &mut impl Implementation,
    ) -> Result<CaseResult> {
        let document = self.document(case)?;
        let mut trend = read_csv(self.directory.join(&case.trend))?;
        let reference = trend
            .reference
            .ok_or("verification trend requires a y column")?;
        let class = &document.library.classes[&document.root.class];
        for (name, offset) in &case.input_offsets {
            if !offset.is_finite()
                || !class
                    .inputs
                    .iter()
                    .any(|p| p.name == *name && p.data_type == cdl_model::DataType::Real)
            {
                return Err(format!("invalid Real input offset for {name}").into());
            }
            for row in &mut trend.rows {
                *row.inputs
                    .get_mut(name)
                    .ok_or_else(|| format!("missing input {name}"))? += offset;
            }
        }
        implementation.initialize(&document, &case.output)?;
        let mut previous = None;
        let mut actual = Vec::with_capacity(trend.rows.len());
        for row in trend.rows {
            let dt = previous.map_or(0., |t| row.time - t);
            previous = Some(row.time);
            let value = implementation.step(&row.inputs, dt)?;
            actual.push(Sample {
                time: row.time,
                value,
            });
        }
        let comparison = compare(
            &reference,
            &actual,
            case.time_tolerance_s,
            case.value_tolerance,
        )?;
        Ok(CaseResult {
            id: case.id.clone(),
            actual,
            comparison,
        })
    }
}
