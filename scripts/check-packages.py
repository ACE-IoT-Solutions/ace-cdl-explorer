#!/usr/bin/env python3
"""Dry-run publishing, then test/install only the extracted publication archives.

Requires Cargo with workspace publishing support. No upload or credentials used.
The temporary workspace's patches stand in for the not-yet-published first release;
all CDL sources are extracted archives, never paths back into this repository.
"""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
PACKAGES = ["cdl-model", "cdl-cxf", "cdl-blocks", "cdl-compat", "cdl-exec", "cdl-verify", "ace-building-sim", "cdl-cli"]


def command(*args, cwd=ROOT):
    subprocess.run(args, cwd=cwd, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--allow-dirty", action="store_true", help="allow local uncommitted release preparation")
    parser.add_argument("--semver", action="store_true", help="compare workspace APIs against the exact publication archives")
    args = parser.parse_args()
    selection = [arg for name in PACKAGES for arg in ("-p", name)]
    command("cargo", "publish", "--dry-run", "--locked", *(["--allow-dirty"] if args.allow_dirty else []), *selection)
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=ROOT))
    versions = {p["name"]: p["version"] for p in metadata["packages"]}
    with tempfile.TemporaryDirectory(prefix="cdl-packaged-consumer-") as temporary:
        isolated = Path(temporary).resolve()
        members = []
        for name in PACKAGES:
            member = f"{name}-{versions[name]}"
            members.append(member)
            package_directory = Path(metadata["target_directory"]) / "package"
            candidates = [package_directory / f"{member}.crate", package_directory / "tmp-crate" / f"{member}.crate"]
            archive = next((path for path in candidates if path.is_file()), None)
            if archive is None:
                raise ValueError(f"Cargo did not produce an archive for {name}")
            with tarfile.open(archive) as tar:
                tar.extractall(isolated, filter="data")
            manifest = tomllib.loads((isolated / member / "Cargo.toml").read_text())
            for section in ("dependencies", "dev-dependencies", "build-dependencies"):
                for dependency in manifest.get(section, {}).values():
                    if isinstance(dependency, dict) and "path" in dependency:
                        raise ValueError(f"published manifest retains a source path: {name}")
            rust_version = manifest["package"]["rust-version"]
            readme = (isolated / member / manifest["package"]["readme"]).read_text()
            if rust_version not in readme:
                raise ValueError(f"README does not state the package MSRV: {name}")
            license_file = isolated / member / manifest["package"]["license-file"]
            if not license_file.is_file():
                raise ValueError(f"missing packaged license: {name}")
        manifest = "[workspace]\nresolver = \"2\"\nmembers = " + json.dumps(members) + "\n\n[patch.crates-io]\n"
        manifest += "\n".join(f'{name} = {{ path = "{member}" }}' for name, member in zip(PACKAGES, members)) + "\n"
        (isolated / "Cargo.toml").write_text(manifest)
        command("cargo", "generate-lockfile", "--offline", cwd=isolated)
        command("cargo", "test", "--workspace", "--all-features", "--release", "--locked", "--offline", cwd=isolated)
        if args.semver:
            # Build rustdoc JSON inside each real workspace. The semver tool's
            # synthetic probe manifests cannot resolve first-release dependencies
            # from a not-yet-published registry; direct JSON avoids that assumption.
            environment = dict(os.environ, RUSTC_BOOTSTRAP="1")
            for name in PACKAGES[:-1]:
                print(f"Archive API equivalence: {name}", flush=True)
                package = next(p for p in metadata["packages"] if p["name"] == name)
                library = next(t["name"] for t in package["targets"] if "lib" in t["kind"])
                rustdocs = []
                for label, workspace in (("current", ROOT), ("archive", isolated)):
                    target = isolated / "api" / label
                    subprocess.run(["cargo", "rustdoc", "-p", name, "--lib", "--all-features", "--locked",
                                    "--offline", "--target-dir", str(target), "--", "-Z", "unstable-options",
                                    "--output-format", "json"], cwd=workspace, env=environment, check=True)
                    rustdocs.append(str(target / "doc" / f"{library}.json"))
                command("cargo", "semver-checks", "--current-rustdoc", rustdocs[0],
                        "--baseline-rustdoc", rustdocs[1], "--release-type", "patch")
        command("cargo", "install", "--path", str(isolated / members[-1]), "--root", str(isolated / "installed"),
                "--locked", "--offline", cwd=isolated)
        executable = str(isolated / "installed/bin/cdl")
        fixture = ROOT / "crates/cdl-integration-tests/tests/cxf/fixtures/CoolingCoil.jsonld"
        shutil.copyfile(fixture, isolated / "CoolingCoil.jsonld")
        command(executable, "validate", "CoolingCoil.jsonld", "--format", "json", cwd=isolated)
        command(executable, "convert", "CoolingCoil.jsonld", "CoolingCoil.json", cwd=isolated)
        command(executable, "validate", "CoolingCoil.json", cwd=isolated)
        print("All eight publication archives passed isolated tests; installed CLI validated and converted upstream CoolingCoil.")


if __name__ == "__main__":
    main()
