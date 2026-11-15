#!/usr/bin/env python3
"""Publish a matching CDL release, safely resuming partial uploads and cooldowns."""

import argparse
import email.utils
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import time
import urllib.error

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("verify_published", ROOT / "scripts/verify-published.py")
VERIFY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFY)


def published(package, version):
    try:
        VERIFY.fetch(f"https://crates.io/api/v1/crates/{package}/{version}")
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        raise
    # A CDN/archive 404 is not evidence that an existing version can be uploaded.
    # Once metadata exists, all download/source errors must stop publication.
    return VERIFY.verify_archive(package, VERIFY.PACKAGES[package], version)


def publish_one(package, version, allow_dirty=False, check=False):
    if published(package, version):
        print(f"{package} {version}: already published; registry checksum and source match", flush=True)
        return
    if check:
        print(f"{package} {version}: not published; upload required", flush=True)
        return
    arguments = ["cargo", "publish", "--locked", "-p", package]
    if allow_dirty:
        arguments.append("--allow-dirty")
    while True:
        result = subprocess.run(arguments, cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        print(result.stdout, flush=True)
        if result.returncode == 0:
            if not published(package, version):
                raise RuntimeError(f"{package}: upload returned success but registry version is missing")
            return
        match = re.search(r"Please try again after (.+? GMT)", result.stdout)
        if "status 429" not in result.stdout or not match:
            # Do not blindly repeat a failed upload: it may have reached the registry.
            if published(package, version):
                print(f"{package}: registry confirms matching upload despite client error", flush=True)
                return
            raise subprocess.CalledProcessError(result.returncode, arguments)
        retry_at = email.utils.parsedate_to_datetime(match.group(1)).timestamp() + 2
        if retry_at <= time.time():
            raise RuntimeError("registry supplied an expired retry time; inspect before retrying")
        print(f"{package}: respecting crates.io cooldown until {match.group(1)}", flush=True)
        while time.time() < retry_at:
            time.sleep(min(30, max(0, retry_at - time.time())))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--allow-dirty", action="store_true")
    parser.add_argument("--check", action="store_true", help="inspect published sources and report pending uploads without publishing")
    args = parser.parse_args()
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=ROOT))
    versions = {p["name"]: p["version"] for p in metadata["packages"] if p["name"] in VERIFY.PACKAGES}
    if set(versions) != set(VERIFY.PACKAGES) or any(version != args.version for version in versions.values()):
        raise ValueError("requested release version must match all eight package versions")
    for package in VERIFY.PACKAGES:
        publish_one(package, args.version, args.allow_dirty, args.check)


if __name__ == "__main__":
    main()
