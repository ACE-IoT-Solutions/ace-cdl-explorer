//! Native CDL command line tools.
#![deny(missing_docs)]
use cdl_verify::{CdlImplementation, Result};
use clap::{Parser, Subcommand, ValueEnum};
use std::{fs, io, path::PathBuf};

#[derive(Parser)]
#[command(
    name = "cdl",
    version,
    about = "Validate, convert, run and verify CDL controllers"
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Text,
    Json,
}
#[derive(Clone, Copy, ValueEnum)]
enum GraphFormat {
    Json,
    Cxf,
}
#[derive(Subcommand)]
enum Command {
    /// Check structural validity (unsupported execution classes may still be valid).
    Validate {
        file: PathBuf,
        #[arg(long, value_enum, default_value = "text")]
        format: Format,
    },
    /// Convert simplified JSON and compacted CXF JSON-LD.
    Convert {
        input: PathBuf,
        output: PathBuf,
        #[arg(long, value_enum)]
        to: Option<GraphFormat>,
    },
    /// Replay a regularly sampled input CSV and write selected outputs to stdout.
    Run {
        graph: PathBuf,
        #[arg(long)]
        inputs: PathBuf,
        #[arg(long)]
        dt: f64,
        #[arg(long)]
        trace: Vec<String>,
    },
    /// Replay a pinned version-two verification suite with the native funnel.
    Verify {
        suite: PathBuf,
        #[arg(long)]
        output_dir: Option<PathBuf>,
        #[arg(long, value_enum, default_value = "text")]
        format: Format,
    },
    /// List core class declarations and execution availability.
    Blocks {
        #[arg(long, value_enum, default_value = "text")]
        format: Format,
    },
}
fn document(path: &PathBuf) -> Result<cdl_model::Document> {
    cdl_verify::load_document(&fs::read_to_string(path)?).map_err(|d| {
        d.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ")
            .into()
    })
}
fn run(args: Args) -> Result<bool> {
    match args.command {
        Command::Validate { file, format } => {
            let diagnostics = match cdl_verify::load_document(&fs::read_to_string(file)?) {
                Ok(document) => cdl_model::validate(&document),
                Err(d) => d,
            };
            match format {
                Format::Json => println!("{}", serde_json::to_string_pretty(&diagnostics)?),
                Format::Text => {
                    for d in &diagnostics {
                        println!("{d}");
                    }
                    if diagnostics.is_empty() {
                        println!("Valid");
                    }
                }
            }
            Ok(diagnostics.is_empty())
        }
        Command::Convert { input, output, to } => {
            let doc = document(&input)?;
            let target = to
                .or_else(|| match output.extension().and_then(|e| e.to_str()) {
                    Some("jsonld") => Some(GraphFormat::Cxf),
                    Some("json") => Some(GraphFormat::Json),
                    _ => None,
                })
                .ok_or("use .json or .jsonld for the output, or specify --to json|cxf")?;
            let text = match target {
                GraphFormat::Cxf => cxf::to_json(&doc),
                GraphFormat::Json => cdl_compat::to_json(&doc),
            }
            .map_err(|d| {
                d.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ")
            })?;
            fs::write(output, text)?;
            Ok(true)
        }
        Command::Run {
            graph,
            inputs,
            dt,
            trace,
        } => {
            if !dt.is_finite() || dt <= 0. {
                return Err("--dt must be finite and positive".into());
            }
            let doc = document(&graph)?;
            let class = &doc.library.classes[&doc.root.class];
            let names = if trace.is_empty() {
                class
                    .outputs
                    .iter()
                    .map(|p| p.name.clone())
                    .collect::<Vec<_>>()
            } else {
                trace
            };
            if names.is_empty() {
                return Err("graph has no outputs; select a --trace path".into());
            }
            let trend = cdl_verify::read_csv(inputs)?;
            let program =
                cdl_exec::Program::compile(&doc, &cdl_blocks::Registry::new(doc.library.clone()))?;
            let output_handles = names
                .iter()
                .map(|name| program.output(name).or_else(|_| program.trace(name)))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let input_handles = class
                .inputs
                .iter()
                .map(|p| Ok((p.name.clone(), program.input(&p.name)?)))
                .collect::<Result<Vec<_>>>()?;
            let mut state = program.instantiate();
            let mut writer = csv::Writer::from_writer(io::stdout().lock());
            writer
                .write_record(std::iter::once("time_s").chain(names.iter().map(String::as_str)))?;
            let mut previous: Option<f64> = None;
            for row in trend.rows {
                if let Some(t) = previous {
                    if ((row.time - t) - dt).abs() > 1e-9 * dt.max(1.) {
                        return Err("CSV sample interval differs from --dt".into());
                    }
                }
                let duration = if previous.is_none() { 0. } else { dt };
                previous = Some(row.time);
                let mut fields = vec![row.time.to_string()];
                for (name, handle) in &input_handles {
                    let &value = row
                        .inputs
                        .get(name)
                        .ok_or_else(|| format!("missing input {name}"))?;
                    let scalar = cdl_verify::input_scalar(value, handle.data_type())?;
                    state.set(handle, scalar)?;
                }
                state.step(duration)?;
                for handle in &output_handles {
                    let value = match state.get(handle)? {
                        cdl_exec::Scalar::Real(v) => v,
                        cdl_exec::Scalar::Integer(v) => v as f64,
                        cdl_exec::Scalar::Boolean(v) => {
                            if v {
                                1.
                            } else {
                                0.
                            }
                        }
                    };
                    fields.push(value.to_string());
                }
                writer.write_record(fields)?;
            }
            writer.flush()?;
            Ok(true)
        }
        Command::Verify {
            suite,
            output_dir,
            format,
        } => {
            let suite = cdl_verify::Suite::load(suite)?;
            if let Some(dir) = &output_dir {
                fs::create_dir_all(dir)?;
            }
            let mut reports = Vec::new();
            for case in &suite.cases {
                let result = suite
                    .run_case(case, &mut CdlImplementation::default())
                    .map_err(|e| format!("{}: {e}", case.id))?;
                if let Some(dir) = &output_dir {
                    let mut writer = csv::Writer::from_path(dir.join(format!("{}.csv", case.id)))?;
                    writer.write_record(["time_s", "y"])?;
                    for sample in &result.actual {
                        writer.write_record([sample.time.to_string(), sample.value.to_string()])?;
                    }
                    writer.flush()?;
                }
                reports.push(result);
            }
            let passed = reports.iter().all(|r| r.comparison.passed);
            if let Some(dir) = &output_dir {
                fs::write(
                    dir.join("native-summary.json"),
                    serde_json::to_vec_pretty(&reports)?,
                )?;
            }
            match format {
                Format::Json => println!("{}", serde_json::to_string_pretty(&reports)?),
                Format::Text => {
                    println!("RESULT  CASE  SAMPLES  VALUE_EXCESS  TIME_EXCESS");
                    for r in reports {
                        println!(
                            "{}  {}  {}  {:.6e}  {}",
                            if r.comparison.passed { "PASS" } else { "FAIL" },
                            r.id,
                            r.actual.len(),
                            r.comparison.max_value_excess,
                            r.comparison
                                .max_time_excess
                                .map(|v| format!("{v:.6e}"))
                                .unwrap_or_else(|| "unbounded".into())
                        );
                    }
                }
            }
            Ok(passed)
        }
        Command::Blocks { format } => {
            let registry = cdl_blocks::Registry::new(cxf::core_library());
            let catalog = registry.catalog();
            match format {
                Format::Json => println!("{}", serde_json::to_string_pretty(&catalog)?),
                Format::Text => {
                    let mut implemented = 0;
                    for (class, available) in registry.standard_classes() {
                        implemented += usize::from(available);
                        println!(
                            "{}  {}",
                            if available {
                                "supported"
                            } else {
                                "import only"
                            },
                            class.id
                        );
                    }
                    println!(
                        "{implemented}/{} core classes supported",
                        catalog.classes.len()
                    );
                }
            }
            Ok(true)
        }
    }
}
fn main() {
    match run(Args::parse()) {
        Ok(true) => {}
        Ok(false) => std::process::exit(1),
        Err(e) => {
            eprintln!("cdl: {e}");
            std::process::exit(2);
        }
    }
}
