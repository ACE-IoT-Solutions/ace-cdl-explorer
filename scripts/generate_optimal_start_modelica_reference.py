#!/usr/bin/env python3
"""Extract a regular trend from the checked-in OpenModelica event trace.

The raw result is produced by ACEOptimalStartValidation using the pinned
Buildings OptimalStartCalculation source. At event times OpenModelica writes
several rows; the final row is the settled value for the 60-second sample.
"""

import csv
import hashlib
import io
import math
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VERIFY = ROOT / "verification"
SOURCE_SHA = "b3afc892b070d712ee116f330ae1d41b8737c1dad58c45634dd0ba61f4086c0a"
CASES = (
    ("g36_optimal_start", "g36_optimal_start_validation.mo",
     "5f83e3b3b2474aa950438fe0590341c3397e2f006338a2795931111c23553686",
     "g36_optimal_start_openmodelica_raw.csv",
     "aa41a337597c902d1913414b9a9f64255234e76a31c4d22fac6193d36755a74a"),
    ("g36_optimal_start_no_recovery", "g36_optimal_start_no_recovery_validation.mo",
     "ea7af1a74ea61e81912bca5c8a136e1f89d505f0be5d1e84af64a1ce19771bcd",
     "g36_optimal_start_no_recovery_openmodelica_raw.csv",
     "cd105cf0baa0720eff85680760afe887374537bb12fc03539512cd588b3bfe74"),
)
STOP = 259200
STEP = 60


def verify_hash(name: str, expected: str) -> None:
    actual = hashlib.sha256((VERIFY / name).read_bytes()).hexdigest()
    if actual != expected:
        raise ValueError(f"{name}: pinned reference checksum changed")


def extract(raw: Path) -> dict[str, str]:

    samples: dict[int, dict[str, str]] = {}
    with raw.open(newline="", encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            time = float(row["time"])
            tick = round(time / STEP)
            if abs(time - tick * STEP) <= 1e-7 and 0 <= tick * STEP <= STOP:
                samples[tick * STEP] = row
    expected_times = list(range(0, STOP + STEP, STEP))
    if sorted(samples) != expected_times:
        raise ValueError("OpenModelica trace does not cover every 60-second sample")

    generated: dict[str, str] = {}
    for output, column in (("tOpt", "calculation.tOpt"), ("optOn", "calculation.optOn")):
        with io.StringIO(newline="") as handle:
            writer = csv.writer(handle, lineterminator="\n")
            writer.writerow(("time_s", "TDif", "staCal", "tNexOcc", "y"))
            for time in expected_times:
                row = samples[time]
                fields = (row["calculation.TDif"], row["calculation.staCal"],
                          row["calculation.tNexOcc"], row[column])
                if not all(math.isfinite(float(value)) for value in fields):
                    raise ValueError(f"OpenModelica trace has nonfinite value at {time} s")
                writer.writerow((time, *fields))
            generated[output] = handle.getvalue()
    return generated


def main() -> None:
    if sys.argv[1:] not in ([], ["--check"]):
        raise SystemExit("usage: generate_optimal_start_modelica_reference.py [--check]")
    verify_hash("g36_optimal_start_calculation_modelica.mo", SOURCE_SHA)
    for prefix, model, model_sha, raw, raw_sha in CASES:
        verify_hash(model, model_sha)
        verify_hash(raw, raw_sha)
        for output, content in extract(VERIFY / raw).items():
            target = VERIFY / f"{prefix}_modelica_{output}.csv"
            if sys.argv[1:] == ["--check"]:
                if target.read_text(encoding="utf-8") != content:
                    raise SystemExit(f"{target} is out of date")
            else:
                target.write_text(content, encoding="utf-8", newline="")


if __name__ == "__main__":
    main()
