#!/usr/bin/env python3
"""Cross-check absolute funnel interpolation, extrema and failing traces.

Requires the pyfunnel environment used by verify-obc.py. The C intersection solver
has small floating-point differences: excesses must agree within 2e-8 and pass/fail
must agree at the existing 1e-10 threshold. No reference files are updated.
"""

import csv
import json
from pathlib import Path
import random
import subprocess
import tempfile

import pyfunnel

ROOT = Path(__file__).resolve().parents[1]


def main() -> None:
    subprocess.run(
        ["cargo", "build", "--release", "-p", "cdl-verify", "--example", "compare_funnel"],
        cwd=ROOT, check=True,
    )
    rng = random.Random(1731)
    cases = []
    expected = []
    with tempfile.TemporaryDirectory(prefix="cdl-funnel-cross-check-") as temporary:
        for index in range(100):
            times = [float(t) for t in range(12)]
            values = [rng.uniform(-5, 5) for _ in times]
            test_times = [0.0] + sorted(rng.uniform(0, 11) for _ in range(18)) + [11.0]
            test_values = [rng.uniform(-7, 7) for _ in test_times]
            if index % 4 == 0:
                test_values = []
                for time in test_times:
                    segment = min(int(time), len(times) - 2)
                    fraction = time - times[segment]
                    test_values.append(values[segment] + fraction * (values[segment + 1] - values[segment]))
            time_tolerance = rng.choice([0.0, 0.1, 0.5, 1.0, 3.0])
            value_tolerance = rng.choice([0.0, 0.1, 1.0])
            cases.append({
                "reference": [{"time": x, "value": y} for x, y in zip(times, values)],
                "test": [{"time": x, "value": y} for x, y in zip(test_times, test_values)],
                "time_tolerance": time_tolerance,
                "value_tolerance": value_tolerance,
            })
            output = Path(temporary) / str(index)
            pyfunnel.compareAndReport(
                xReference=times, yReference=values, xTest=test_times, yTest=test_values,
                outputDirectory=str(output), atolx=time_tolerance, atoly=value_tolerance,
            )
            with (output / "errors.csv").open(newline="", encoding="utf-8") as handle:
                expected.append([float(row["y"]) for row in csv.DictReader(handle)])
        actual = json.loads(subprocess.check_output(
            [str(ROOT / "target/release/examples/compare_funnel")],
            input=json.dumps(cases).encode(), cwd=ROOT,
        ))
        if len(actual) != len(expected):
            raise ValueError("native comparison omitted experiments")
        if not any(item["passed"] for item in actual) or all(item["passed"] for item in actual):
            raise ValueError("cross-check must include passing and failing experiments")
        maximum = 0.0
        for index, (reference_errors, result) in enumerate(zip(expected, actual)):
            if len(reference_errors) != len(result["errors"]):
                raise ValueError(f"experiment {index}: error sample counts differ")
            delta = max(abs(a - b) for a, b in zip(reference_errors, result["errors"]))
            maximum = max(maximum, delta)
            if delta > 2e-8 or (max(reference_errors) <= 1e-10) != result["passed"]:
                raise ValueError(f"experiment {index}: native and pyfunnel disagree ({delta})")
        print(f"100 seeded experiments, 2000 samples agree; maximum error delta {maximum:.6g}")


if __name__ == "__main__":
    main()
