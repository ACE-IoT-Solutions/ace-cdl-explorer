//! Batch comparison bridge for the independent pyfunnel cross-check.
use serde::Deserialize;
#[derive(Deserialize)]
struct Input {
    reference: cdl_verify::Trace,
    test: cdl_verify::Trace,
    time_tolerance: f64,
    value_tolerance: f64,
}
fn main() -> cdl_verify::Result<()> {
    let cases: Vec<Input> = serde_json::from_reader(std::io::stdin().lock())?;
    let results = cases
        .iter()
        .map(|c| cdl_verify::compare(&c.reference, &c.test, c.time_tolerance, c.value_tolerance))
        .collect::<cdl_verify::Result<Vec<_>>>()?;
    serde_json::to_writer(std::io::stdout().lock(), &results)?;
    Ok(())
}
