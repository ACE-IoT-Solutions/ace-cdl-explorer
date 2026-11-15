use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("cdl-cli-test-{}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn command(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_cdl"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn external_user_validate_convert_run_and_failure_codes() {
    let f = Fixture::new();
    fs::write(f.0.join("add.json"),r#"{"name":"add","block_type":"Buildings.Controls.OBC.CDL.Reals.Add","inputs":[{"name":"u1","type":"Real"},{"name":"u2","type":"Real"}],"outputs":[{"name":"y","type":"Real"}]}"#).unwrap();
    let valid = f.command(&["validate", "add.json", "--format", "json"]);
    assert!(valid.status.success(), "{:?}", valid);
    assert_eq!(String::from_utf8(valid.stdout).unwrap().trim(), "[]");
    let converted = f.command(&["convert", "add.json", "add.jsonld"]);
    assert!(converted.status.success(), "{:?}", converted);
    assert!(f.command(&["validate", "add.jsonld"]).status.success());
    assert!(f
        .command(&["convert", "add.jsonld", "back.json"])
        .status
        .success());
    fs::write(f.0.join("inputs.csv"), "time_s,u1,u2\n0,2,3\n1,4,5\n").unwrap();
    let replay = f.command(&[
        "run",
        "add.jsonld",
        "--inputs",
        "inputs.csv",
        "--dt",
        "1",
        "--trace",
        "y",
    ]);
    assert!(replay.status.success(), "{:?}", replay);
    assert_eq!(
        String::from_utf8(replay.stdout).unwrap(),
        "time_s,y\n0,5\n1,9\n"
    );
    assert!(!f
        .command(&["run", "add.json", "--inputs", "inputs.csv", "--dt", "2"])
        .status
        .success());
    fs::write(f.0.join("bad.json"), "{").unwrap();
    let invalid = f.command(&["validate", "bad.json", "--format", "json"]);
    assert_eq!(invalid.status.code(), Some(1));
    let diagnostics: serde_json::Value = serde_json::from_slice(&invalid.stdout).unwrap();
    assert!(!diagnostics.as_array().unwrap().is_empty());
    fs::write(
        f.0.join("reference.csv"),
        "time_s,u1,u2,y\n0,2,3,5\n1,4,5,9\n",
    )
    .unwrap();
    let mut suite = serde_json::json!({"version":2,"reference_files":{"reference.csv":"2bb319f392c4f2c0dc9a4cad8d676802853b8f00458c19c154fcb5801d4b6ea9"},"cases":[{
        "id":"add","graph_file":"add.jsonld","trend":"reference.csv","time_tolerance_s":0,"value_tolerance":0
    }]});
    fs::write(f.0.join("suite.json"), suite.to_string()).unwrap();
    let verified = f.command(&["verify", "suite.json", "--format", "json"]);
    assert!(verified.status.success(), "{:?}", verified);
    let reports: serde_json::Value = serde_json::from_slice(&verified.stdout).unwrap();
    assert_eq!(reports[0]["comparison"]["passed"], true);
    fs::write(
        f.0.join("reference.csv"),
        "time_s,u1,u2,y\n0,2,3,4\n1,4,5,9\n",
    )
    .unwrap();
    assert_eq!(f.command(&["verify", "suite.json"]).status.code(), Some(2));
    suite["reference_files"]["reference.csv"] =
        "44b894cb9da862d61fa436b2a89bfdce3244c1fc0bd3afe4f87c02ed09f1fd61".into();
    fs::write(f.0.join("suite.json"), suite.to_string()).unwrap();
    let failed = f.command(&["verify", "suite.json", "--format", "json"]);
    assert_eq!(failed.status.code(), Some(1));
    let reports: serde_json::Value = serde_json::from_slice(&failed.stdout).unwrap();
    assert_eq!(reports[0]["comparison"]["max_value_excess"], 1.0);
    let blocks = f.command(&["blocks", "--format", "json"]);
    assert!(blocks.status.success());
    let catalog: serde_json::Value = serde_json::from_slice(&blocks.stdout).unwrap();
    assert_eq!(catalog["classes"].as_object().unwrap().len(), 132);
}
