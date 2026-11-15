// Enable fixture/original-engine regressions only in the full source repository.
// Published archives retain independent unit/API tests and need no repository assets.
fn main() {
    println!("cargo:rustc-check-cfg=cfg(ace_building_sim_repository_tests)");
    let manifest = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let repository = manifest.join("../..");
    if repository
        .join("crates/cdl-runtime-oracle/Cargo.toml")
        .is_file()
        && repository
            .join("web/public/examples/four_pipe_g36.json")
            .is_file()
    {
        println!("cargo:rustc-cfg=ace_building_sim_repository_tests");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
