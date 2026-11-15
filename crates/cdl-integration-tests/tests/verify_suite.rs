use cdl_verify::{CdlImplementation, Suite};
use std::path::Path;
#[test]
fn all_pinned_repository_cases_match_native_funnel() {
    let suite =
        Suite::load(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../verification/suite.json"))
            .unwrap();
    assert_eq!(suite.cases.len(), 143);
    for case in &suite.cases {
        let result = suite
            .run_case(case, &mut CdlImplementation::default())
            .unwrap_or_else(|e| panic!("{}: {e}", case.id));
        assert!(
            result.comparison.passed,
            "{}: {:?}",
            case.id, result.comparison
        );
    }
}
