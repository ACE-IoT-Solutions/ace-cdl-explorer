use cdl_verify::{CdlImplementation, Suite};
use std::fs;
#[test]
fn rejects_tampered_references_before_execution() {
    let path = std::env::temp_dir().join(format!("cdl-verify-pin-test-{}", std::process::id()));
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join("trend.csv"), "time_s,y\n0,1\n").unwrap();
    let suite = serde_json::json!({"version":2,"reference_files":{"trend.csv":"0".repeat(64)},"cases":[{
        "id":"pin","graph":{},"trend":"trend.csv","time_tolerance_s":0,"value_tolerance":0
    }]});
    fs::write(path.join("suite.json"), suite.to_string()).unwrap();
    let result = Suite::load(path.join("suite.json"));
    fs::remove_dir_all(path).unwrap();
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("pinned reference changed"));
}

#[test]
fn cxf_suite_and_another_implementation_use_the_same_experiment() {
    use cdl_model::Document;
    use cdl_verify::{Implementation, Result};
    use sha2::{Digest, Sha256};
    use std::collections::BTreeMap;
    struct Other {
        durations: Vec<f64>,
    }
    impl Implementation for Other {
        fn initialize(&mut self, _document: &Document, output: &str) -> Result<()> {
            assert_eq!(output, "y");
            self.durations.clear();
            Ok(())
        }
        fn step(&mut self, inputs: &BTreeMap<String, f64>, dt: f64) -> Result<f64> {
            self.durations.push(dt);
            Ok(inputs["u1"] + inputs["u2"])
        }
    }
    let graph = r#"{"name":"add","block_type":"Buildings.Controls.OBC.CDL.Reals.Add","inputs":[{"name":"u1","type":"Real"},{"name":"u2","type":"Real"}],"outputs":[{"name":"y","type":"Real"}]}"#;
    let document = cdl_verify::load_document(graph).unwrap();
    let cxf = cxf::to_value(&document).unwrap();
    let path = std::env::temp_dir().join(format!("cdl-verify-cxf-test-{}", std::process::id()));
    fs::create_dir_all(&path).unwrap();
    let trend = "time_s,u1,u2,y\n0,2,3,5\n1,4,5,9\n";
    fs::write(path.join("trend.csv"), trend).unwrap();
    let suite = serde_json::json!({"version":2,"reference_files":{"trend.csv":format!("{:x}",Sha256::digest(trend))},"cases":[{
        "id":"cxf","graph":cxf,"trend":"trend.csv","time_tolerance_s":0,"value_tolerance":0
    }]});
    fs::write(path.join("suite.json"), suite.to_string()).unwrap();
    let suite = Suite::load(path.join("suite.json")).unwrap();
    let case = &suite.cases[0];
    let native = suite
        .run_case(case, &mut CdlImplementation::default())
        .unwrap();
    let mut other = Other { durations: vec![] };
    let remote = suite.run_case(case, &mut other).unwrap();
    assert_eq!(native.actual, remote.actual);
    assert!(remote.comparison.passed);
    assert_eq!(other.durations, vec![0., 1.]);
    fs::remove_dir_all(path).unwrap();
}
