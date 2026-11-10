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
RAW = VERIFY / "g36_optimal_start_openmodelica_raw.csv"
SOURCE_SHA = "b3afc892b070d712ee116f330ae1d41b8737c1dad58c45634dd0ba61f4086c0a"
MODEL_SHA = "5f83e3b3b2474aa950438fe0590341c3397e2f006338a2795931111c23553686"
RAW_SHA = "aa41a337597c902d1913414b9a9f64255234e76a31c4d22fac6193d36755a74a"
STOP = 259200
STEP = 60


def verify_hash(name: str, expected: str) -> None:
    actual = hashlib.sha256((VERIFY / name).read_bytes()).hexdigest()
    if actual != expected:
        raise ValueError(f"{name}: pinned reference checksum changed")


def extract() -> dict[str, str]:
    for name, expected in (
        ("g36_optimal_start_calculation_modelica.mo", SOURCE_SHA),
        ("g36_optimal_start_validation.mo", MODEL_SHA),
        (RAW.name, RAW_SHA),
    ):
        verify_hash(name, expected)

    samples: dict[int, dict[str, str]] = {}
    with RAW.open(newline="", encoding="utf-8") as handle:
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
    for output, content in extract().items():
        target = VERIFY / f"g36_optimal_start_modelica_{output}.csv"
        if sys.argv[1:] == ["--check"]:
            if target.read_text(encoding="utf-8") != content:
                raise SystemExit(f"{target} is out of date")
        else:
            target.write_text(content, encoding="utf-8", newline="")


if __name__ == "__main__":
    main()
