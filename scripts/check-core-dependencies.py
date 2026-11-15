#!/usr/bin/env python3
"""Reject app dependencies in the reusable CDL crates, including transitive ones."""

import json
from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[1]
CORE = {"cdl-model", "cdl-blocks", "cdl-exec", "cdl-cxf", "cdl-compat", "cdl-verify"}


def main() -> None:
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"],
        cwd=ROOT, text=True,
    ))
    members = set(metadata["workspace_members"])
    for package in metadata["packages"]:
        if package["id"] not in members or package["name"] not in CORE | {"ace-building-sim"}:
            continue
        tree = subprocess.check_output(
            ["cargo", "tree", "--locked", "--package", package["name"],
             "--edges", "normal,build", "--target", "all", "--all-features", "--prefix", "none",
             "--format", "{p}"], cwd=ROOT, text=True,
        )
        forbidden = sorted({line.split()[0] for line in tree.splitlines() if line and (
            line.split()[0].startswith(("wasm-bindgen", "arrow-"))
            or (package["name"] in CORE and line.split()[0] in {"ace-building-sim", "ace-cdl-wasm", "ace-cdl-runtime"})
        )})
        if forbidden:
            raise SystemExit(f"{package['name']}: forbidden core dependencies: {', '.join(forbidden)}")
        print(f"{package['name']}: dependency boundary passed")


if __name__ == "__main__":
    main()
