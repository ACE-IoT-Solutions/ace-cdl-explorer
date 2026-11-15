//! Native format-conversion example used by cross-language conformance checks.
use std::{error::Error, fs};
fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let input = args.next().ok_or("input path required")?;
    let output = args.next().ok_or("output path required")?;
    let source: serde_json::Value = serde_json::from_str(&fs::read_to_string(input)?)?;
    let doc = if source.get("@graph").is_some() {
        cxf::from_value(&source, &cxf::core_library())
    } else {
        cdl_compat::from_value(&source, Default::default())
    }
    .map_err(|e| format!("{e:?}"))?;
    let value = if args.next().as_deref() == Some("legacy") {
        cdl_compat::to_value(&doc)
    } else {
        cxf::to_value(&doc)
    }
    .map_err(|e| format!("{e:?}"))?;
    fs::write(output, serde_json::to_string_pretty(&value)?)?;
    Ok(())
}
