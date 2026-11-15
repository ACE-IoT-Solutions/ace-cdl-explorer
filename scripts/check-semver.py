#!/usr/bin/env python3
"""Check public APIs against the latest reachable cdl-v* release tag.

The initial release has no historical baseline. check-packages.py --semver
separately compares its workspace API with the exact publication archives.
"""
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PACKAGES = ["cdl-model", "cdl-cxf", "cdl-blocks", "cdl-compat", "cdl-exec", "cdl-verify"]


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def main():
    head = git("rev-parse", "HEAD")
    tags = git("tag", "--merged", "HEAD", "--list", "cdl-v*", "--sort=-version:refname").splitlines()
    tags = [tag for tag in tags if re.fullmatch(r"cdl-v\d+\.\d+\.\d+", tag)
            and git("rev-parse", f"{tag}^{{commit}}") != head]
    if not tags:
        print("Initial CDL release: no prior reachable release tag; archive API equivalence is checked separately.")
        return
    for package in PACKAGES:
        subprocess.run(["cargo", "semver-checks", "-p", package, "--baseline-rev", tags[0], "--all-features"],
                       cwd=ROOT, check=True)


if __name__ == "__main__":
    main()
