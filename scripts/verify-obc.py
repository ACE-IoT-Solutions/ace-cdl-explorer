#!/usr/bin/env python3
"""Replay CDL input trends through Rust and compare outputs with LBNL's funnel."""

import csv
import json
import math
import shutil
import subprocess
import sys
from pathlib import Path

import pyfunnel


ROOT = Path(__file__).resolve().parents[1]
SUITE = ROOT / "verification" / "suite.json"
RESULTS = ROOT / "verification" / "results"


def read_trace(path: Path) -> tuple[list[float], list[float]]:
    with path.open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    if not rows:
        raise ValueError(f"{path}: no trace rows")
    time = [float(row["time_s"]) for row in rows]
    values = [float(row["y"]) for row in rows]
    if not all(math.isfinite(value) for value in time + values):
        raise ValueError(f"{path}: nonfinite trace value")
    if any(later <= earlier for earlier, later in zip(time, time[1:])):
        raise ValueError(f"{path}: time must strictly increase")
    return time, values


def main() -> int:
    suite = json.loads(SUITE.read_text(encoding="utf-8"))
    subprocess.run([sys.executable, str(ROOT / "scripts/generate_optimal_start_reference.py"),
                    "--check"], check=True, cwd=ROOT)
    if RESULTS.exists():
        shutil.rmtree(RESULTS)
    actual_dir = RESULTS / "actual"
    actual_dir.mkdir(parents=True)
    subprocess.run(
        ["cargo", "run", "--quiet", "--manifest-path", str(ROOT / "runtime/Cargo.toml"),
         "--bin", "obc_verify", "--", str(SUITE), str(actual_dir)],
        check=True, cwd=ROOT,
    )
    report = []
    for case in suite["cases"]:
        reference_time, reference_y = read_trace(SUITE.parent / case["trend"])
        time_tolerance = float(case["time_tolerance_s"])
        value_tolerance = float(case["value_tolerance"])
        if time_tolerance < 0 or value_tolerance < 0:
            raise ValueError(f"{case['id']}: negative tolerance")
        for engine, suffix in (("dynamic", ""), ("compiled", "_compiled")):
            actual_time, actual_y = read_trace(actual_dir / f"{case['id']}{suffix}.csv")
            if len(reference_time) != len(actual_time):
                raise ValueError(f"{case['id']} {engine}: reference and runtime sample counts differ")
            output = RESULTS / f"{case['id']}{suffix}"
            pyfunnel.compareAndReport(
                xReference=reference_time, yReference=reference_y,
                xTest=actual_time, yTest=actual_y,
                outputDirectory=str(output), atolx=time_tolerance, atoly=value_tolerance,
            )
            with (output / "errors.csv").open(newline="", encoding="utf-8") as handle:
                errors = [abs(float(row["y"])) for row in csv.DictReader(handle)]
            if len(errors) != len(actual_time) or not all(math.isfinite(error) for error in errors):
                raise ValueError(f"{case['id']} {engine}: invalid funnel result")
            maximum = max(errors, default=0.0)
            passed = maximum <= 1e-10
            report.append({"case": case["id"], "engine": engine, "samples": len(errors),
                           "max_excess": maximum, "passed": passed})
            print(f"{'PASS' if passed else 'FAIL'} {case['id']} {engine}: {len(errors)} samples, max excess {maximum:.6g}")
    (RESULTS / "summary.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return 0 if all(item["passed"] for item in report) else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"OBC verification error: {error}", file=sys.stderr)
        sys.exit(2)
