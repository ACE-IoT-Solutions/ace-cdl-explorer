#!/usr/bin/env python3
"""Verify registry archive checksums/source and a clean cargo install of the CLI.

Run against a matching release worktree after publishing all eight packages. This
uses public registry downloads; it never uploads or modifies registry credentials.
"""

import argparse
import concurrent.futures
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import subprocess
import tarfile
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
PACKAGES = {
    "cdl-model": "cdl-model", "cdl-cxf": "cxf", "cdl-blocks": "cdl-blocks",
    "cdl-compat": "cdl-compat", "cdl-exec": "cdl-exec", "cdl-verify": "cdl-verify",
    "ace-building-sim": "ace-building-sim", "cdl-cli": "cdl-cli",
}


def fetch(url):
    request = urllib.request.Request(url, headers={
        "User-Agent": "ace-cdl-explorer release verification (github.com/ACE-IoT-Solutions/ace-cdl-explorer)",
    })
    with urllib.request.urlopen(request, timeout=60) as response:
        return response.read()


def verify_archive(name, directory, version):
    record = json.loads(fetch(f"https://crates.io/api/v1/crates/{name}/{version}"))["version"]
    archive = fetch(f'https://crates.io{record["dl_path"]}')
    checksum = hashlib.sha256(archive).hexdigest()
    if checksum != record["checksum"]:
        raise ValueError(f"{name}: registry archive checksum mismatch")
    count = 0
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
        for entry in tar.getmembers():
            if not entry.isfile():
                continue
            relative = PurePosixPath(entry.name).relative_to(f"{name}-{version}")
            if ".." in relative.parts:
                raise ValueError(f"{name}: invalid archive path")
            if str(relative) in {"Cargo.toml", "Cargo.lock", ".cargo_vcs_info.json"}:
                continue
            source = ROOT / "crates" / directory / ("Cargo.toml" if str(relative) == "Cargo.toml.orig" else relative)
            if not source.is_file() or source.read_bytes() != tar.extractfile(entry).read():
                raise ValueError(f"{name}: published file differs from worktree: {relative}")
            count += 1
    return {
        "name": name, "version": version, "created_at": record["created_at"],
        "checksum": checksum, "source_files_verified": count, "rust_version": record["rust_version"],
        "url": f"https://crates.io/crates/{name}/{version}",
    }


def clean_install(version):
    with tempfile.TemporaryDirectory(prefix="cdl-registry-install-") as temporary:
        isolated = Path(temporary).resolve()
        subprocess.run(["cargo", "install", "cdl-cli", "--version", f"={version}", "--locked",
                        "--root", str(isolated / "installed")], cwd=isolated, check=True)
        executable = str(isolated / "installed/bin/cdl")

        def cdl(*arguments):
            return subprocess.check_output([executable, *arguments], cwd=isolated, text=True)

        if cdl("--version").strip() != f"cdl {version}":
            raise ValueError("installed CLI version differs from release")
        manifest = json.loads((ROOT / "crates/cdl-integration-tests/tests/cxf/fixtures/manifest.json").read_text())
        fixture = next(f for f in manifest["fixtures"] if f["file"] == "CoolingCoil.jsonld")
        payload = fetch(f'https://raw.githubusercontent.com/lbl-srg/modelica-json/{manifest["revision"]}/{fixture["source"]}')
        if hashlib.sha256(payload).hexdigest() != fixture["sha256"]:
            raise ValueError("upstream CoolingCoil fixture checksum mismatch")
        (isolated / "CoolingCoil.jsonld").write_bytes(payload)
        if json.loads(cdl("validate", "CoolingCoil.jsonld", "--format", "json")):
            raise ValueError("published CLI rejected upstream CoolingCoil")
        cdl("convert", "CoolingCoil.jsonld", "CoolingCoil.json")
        cdl("validate", "CoolingCoil.json")
        catalog = json.loads(cdl("blocks", "--format", "json"))
        if len(catalog["classes"]) != 132:
            raise ValueError("published catalog coverage differs")
        graph = {
            "name": "add", "block_type": "Buildings.Controls.OBC.CDL.Reals.Add",
            "inputs": [{"name": "u1", "type": "Real"}, {"name": "u2", "type": "Real"}],
            "outputs": [{"name": "y", "type": "Real"}],
        }
        (isolated / "add.json").write_text(json.dumps(graph))
        trend = "time_s,u1,u2,y\n0,2,3,5\n1,4,5,9\n"
        (isolated / "input.csv").write_text(trend)
        replay = cdl("run", "add.json", "--inputs", "input.csv", "--dt", "1", "--trace", "y")
        if replay != "time_s,y\n0,5\n1,9\n":
            raise ValueError("published CLI replay differs")
        suite = {
            "version": 2, "reference_files": {"input.csv": hashlib.sha256(trend.encode()).hexdigest()},
            "cases": [{"id": "registry_add", "graph_file": "add.json", "trend": "input.csv",
                       "time_tolerance_s": 0, "value_tolerance": 0}],
        }
        (isolated / "suite.json").write_text(json.dumps(suite))
        reports = json.loads(cdl("verify", "suite.json", "--format", "json"))
        if len(reports) != 1 or not reports[0]["comparison"]["passed"]:
            raise ValueError("published CLI verification differs")
    return {"version": version, "upstream_cooling_coil": "passed", "commands": ["validate", "convert", "blocks", "run", "verify"]}


def clean_native_consumer(version):
    """Compile and exercise the public APIs using registry dependencies only."""
    with tempfile.TemporaryDirectory(prefix="cdl-registry-native-") as temporary:
        isolated = Path(temporary).resolve()
        (isolated / "tests").mkdir()
        (isolated / "Cargo.toml").write_text(f'''[package]
name = "registry-native-consumer"
version = "0.0.0"
edition = "2021"
[dependencies]
ace-building-sim = {{ version = "={version}", features = ["checkpoint"] }}
cdl-blocks = "={version}"
cdl-exec = {{ version = "={version}", features = ["serde"] }}
cdl-compat = "={version}"
serde_json = "1"
[features]
default = ["checkpoint"]
checkpoint = []
''')
        (isolated / "tests/native_api.rs").write_bytes(
            (ROOT / "crates/ace-building-sim/tests/native_api.rs").read_bytes())
        subprocess.run(["cargo", "test", "--release"], cwd=isolated, check=True)
    return {"version": version, "source": "crates.io", "plant_step": "passed",
            "invalid_command_atomicity": "passed", "serialized_state_restore": "passed"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--receipt", type=Path)
    args = parser.parse_args()
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        pending = [pool.submit(verify_archive, name, directory, args.version) for name, directory in PACKAGES.items()]
        archives = [future.result() for future in pending]
    for archive in archives:
        print(f'Verified registry archive: {archive["name"]} {args.version}', flush=True)
    installed = clean_install(args.version)
    native = clean_native_consumer(args.version)
    if args.receipt:
        args.receipt.write_text(json.dumps({"packages": archives, "clean_install": installed, "native_consumer": native}, indent=2) + "\n")
    print("All eight registry archives match; clean cargo install passes all five CLI commands with upstream CXF; registry plant/checkpoint consumer passes.")


if __name__ == "__main__":
    main()
