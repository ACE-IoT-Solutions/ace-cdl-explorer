//! Generate frontend contracts from Rust, or check the committed output for drift.
use ace_building_sim::{Scenario, SimulationRow, SimulationTotals, MEASURE_COLUMNS};
use cdl_model::Diagnostic;
use std::{collections::BTreeMap, error::Error, fs, path::Path};
use ts_rs::{Config, TS};

fn files(root: &Path) -> Result<BTreeMap<String, Vec<u8>>, Box<dyn Error>> {
    let mut files = BTreeMap::new();
    if root.exists() {
        for entry in fs::read_dir(root)? {
            let path = entry?.path();
            if path.extension().is_some_and(|s| s == "ts") {
                files.insert(
                    path.file_name().unwrap().to_string_lossy().into_owned(),
                    fs::read(path)?,
                );
            }
        }
    }
    Ok(files)
}
fn main() -> Result<(), Box<dyn Error>> {
    let check = std::env::args().skip(1).any(|arg| arg == "--check");
    let output = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/src/lib/generated");
    let temporary = std::env::temp_dir().join(format!(
        "ace-cdl-types-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir_all(&temporary)?;
    let result = generate(&temporary);
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&temporary);
        return Err(error);
    }
    let generated = files(&temporary)?;
    let current = files(&output)?;
    fs::remove_dir_all(&temporary)?;
    if check {
        if generated != current {
            return Err("Generated TypeScript is stale; run scripts/generate-web.sh".into());
        }
    } else {
        fs::create_dir_all(&output)?;
        for old in current.keys().filter(|name| !generated.contains_key(*name)) {
            fs::remove_file(output.join(old))?;
        }
        for (name, contents) in generated {
            fs::write(output.join(name), contents)?;
        }
    }
    Ok(())
}
fn generate(output: &Path) -> Result<(), Box<dyn Error>> {
    let config = Config::new()
        .with_out_dir(output)
        .with_large_int("number")
        .with_array_tuple_limit(0);
    Scenario::export_all(&config)?;
    SimulationRow::export_all(&config)?;
    SimulationTotals::export_all(&config)?;
    Diagnostic::export_all(&config)?;
    let mut index = String::from("// Generated from Rust by generate-types. Do not edit.\n");
    for name in files(output)?.keys() {
        let stem = name.trim_end_matches(".ts");
        index.push_str(&format!("export type {{ {stem} }} from './{stem}'\n"));
    }
    // The browser's Arrow column list follows the native encoder contract.
    let measures = serde_json::to_string_pretty(MEASURE_COLUMNS)?;
    let row = serde_json::to_value(SimulationRow::default())?;
    let zone_arrays: Vec<_> = row
        .as_object()
        .unwrap()
        .iter()
        .filter(|(key, value)| key.starts_with("zone_") && value.is_array())
        .map(|(key, _)| key)
        .collect();
    let columns = format!("// Generated from Rust. Do not edit.\nexport const measureColumns = {measures} as const\nexport const zoneRowColumns = {} as const\n", serde_json::to_string_pretty(&zone_arrays)?);
    fs::write(output.join("rowColumns.ts"), columns)?;
    fs::write(output.join("index.ts"), index)?;
    Ok(())
}
