use ace_cdl_runtime::{validate, Block, CompiledExecutor, Executor};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::HashMap, error::Error, fs, path::Path};

#[derive(Deserialize)]
struct Suite {
    version: u8,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    graph: Option<Block>,
    graph_file: Option<String>,
    trend: String,
}

fn run_case(case: &Case, suite_dir: &Path, output_dir: &Path) -> Result<usize, Box<dyn Error>> {
    let graph = match (&case.graph, &case.graph_file) {
        (Some(graph), None) => graph.clone(),
        (None, Some(path)) => serde_json::from_str(&fs::read_to_string(path)?)?,
        _ => return Err(format!("{}: specify exactly one graph or graph_file", case.id).into()),
    };
    let issues = validate(&graph);
    if !issues.is_empty() {
        return Err(format!("{}: invalid graph: {}", case.id, issues.join("; ")).into());
    }
    if graph.outputs.len() != 1 || graph.outputs[0].name != "y" {
        return Err(format!(
            "{}: verification currently requires one output named y",
            case.id
        )
        .into());
    }
    let trend = fs::read_to_string(suite_dir.join(&case.trend))?;
    let mut lines = trend.lines();
    let columns: Vec<&str> = lines
        .next()
        .ok_or("empty trend")?
        .split(',')
        .map(str::trim)
        .collect();
    if columns.first() != Some(&"time_s") || !columns.contains(&"y") {
        return Err(format!("{}: trend needs time_s and y columns", case.id).into());
    }
    for port in &graph.inputs {
        if !columns.contains(&port.name.as_str()) {
            return Err(format!("{}: trend lacks input {}", case.id, port.name).into());
        }
    }
    let mut executor = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&graph)?
        .ok_or("verification graph has no compiled CDL implementation")?;
    let mut previous_time = None;
    let mut actual = String::from("time_s,y\n");
    let mut optimized_actual = String::from("time_s,y\n");
    let mut count = 0;
    for (line_number, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split(',').map(str::trim).collect();
        if fields.len() != columns.len() {
            return Err(format!("{}:{}: wrong column count", case.id, line_number + 2).into());
        }
        let get = |name: &str| fields[columns.iter().position(|column| *column == name).unwrap()];
        let time: f64 = get("time_s").parse()?;
        if !time.is_finite() || previous_time.is_some_and(|old| time <= old) {
            return Err(format!("{}:{}: times must increase", case.id, line_number + 2).into());
        }
        let dt = previous_time.map_or(0.0, |old| time - old);
        previous_time = Some(time);
        let mut inputs = HashMap::<String, Value>::new();
        for port in &graph.inputs {
            let raw = get(&port.name);
            let value = if port.kind == "Boolean" {
                json!(match raw {
                    "1" | "true" => true,
                    "0" | "false" => false,
                    _ =>
                        return Err(format!(
                            "{}:{}: invalid Boolean {}",
                            case.id,
                            line_number + 2,
                            port.name
                        )
                        .into()),
                })
            } else {
                let number: f64 = raw.parse()?;
                if !number.is_finite() {
                    return Err(format!("{}:{}: nonfinite input", case.id, line_number + 2).into());
                }
                json!(number)
            };
            inputs.insert(port.name.clone(), value);
        }
        let outputs = executor.execute(&graph, &inputs, dt, "")?;
        let optimized_outputs = compiled.execute(&inputs, dt, false)?.0;
        let value = outputs.get("y").ok_or("runtime omitted y")?;
        let numeric = match value {
            Value::Bool(value) => {
                if *value {
                    1.0
                } else {
                    0.0
                }
            }
            Value::Number(value) => value.as_f64().ok_or("non-numeric output")?,
            _ => return Err("unsupported output type".into()),
        };
        if !numeric.is_finite() {
            return Err(format!("{}:{}: nonfinite output", case.id, line_number + 2).into());
        }
        actual.push_str(&format!("{time},{numeric}\n"));
        let optimized_numeric = optimized_outputs
            .get("y")
            .and_then(Value::as_f64)
            .ok_or("compiled runtime omitted numeric y")?;
        if !optimized_numeric.is_finite() {
            return Err(
                format!("{}:{}: nonfinite compiled output", case.id, line_number + 2).into(),
            );
        }
        optimized_actual.push_str(&format!("{time},{optimized_numeric}\n"));
        count += 1;
    }
    if count == 0 {
        return Err(format!("{}: no trend rows", case.id).into());
    }
    fs::write(output_dir.join(format!("{}.csv", case.id)), actual)?;
    fs::write(
        output_dir.join(format!("{}_compiled.csv", case.id)),
        optimized_actual,
    )?;
    Ok(count)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: obc_verify <suite.json> <output-directory>".into());
    }
    let suite_path = Path::new(&args[1]);
    let suite: Suite = serde_json::from_str(&fs::read_to_string(suite_path)?)?;
    if suite.version != 1 {
        return Err("unsupported verification suite version".into());
    }
    let output_dir = Path::new(&args[2]);
    fs::create_dir_all(output_dir)?;
    let suite_dir = suite_path.parent().unwrap_or(Path::new("."));
    for case in &suite.cases {
        let rows = run_case(case, suite_dir, output_dir)?;
        println!("{}: generated {} output samples", case.id, rows);
    }
    Ok(())
}
