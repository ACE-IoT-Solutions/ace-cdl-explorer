#!/usr/bin/env python3
"""Extract paired OBC low-flow alarm traces from one measured-airflow trajectory."""

import csv
import hashlib
import io
import math
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VERIFY = ROOT / "verification"
RAW = VERIFY / "g36_low_flow_importance_openmodelica_raw.csv"
PINNED = {
    "g36_reheat_alarms_modelica.mo": "4ea2e6e267916e0759423f76c578b01487da6f55d9d0ee5fc18a101e102e96ce",
    "G36LowFlowImportance.mo": "b031cfa929f802ed2f3f1b1158b8de7db5a8f0a550aaec8fba5948beec633f7b",
    "g36_low_flow_importance.mos": "cc5000e966472a6c67e08e76bbc9a4d30363bd3dd2823a19811ca0f20497ca41",
    "g36_low_flow_importance_openmodelica_raw.csv": "7cfc7cadebd3db11ad41f8b2439bdf7c4c9de88474482e85f761d8bb68a6a5e2",
}
STEP = 30
STOP = 3600


def trends() -> dict[Path, str]:
    for name, expected in PINNED.items():
        if hashlib.sha256((VERIFY / name).read_bytes()).hexdigest() != expected:
            raise ValueError(f"pinned low-flow importance reference changed: {name}")
    samples = {}
    with RAW.open(newline="", encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            time = float(row["time"])
            tick = round(time / STEP)
            if abs(time - tick * STEP) < 1e-3 and 0 <= tick * STEP <= STOP:
                samples[tick * STEP] = row
    if sorted(samples) != list(range(0, STOP + STEP, STEP)):
        raise ValueError("OpenModelica low-flow importance trace does not cover 121 samples")
    output = {}
    for name, important in (("active", 1), ("suppressed", 0)):
        with io.StringIO(newline="") as handle:
            writer = csv.writer(handle, lineterminator="\n")
            writer.writerow(("time_s", "setpoint_flow", "measured_flow", "fan_proven",
                             "occupied", "important", "y"))
            levels = set()
            for time, row in sorted(samples.items()):
                flow = float(row["measuredFlow.y"])
                level = int(row[f"{name}.yLowFloAla"])
                if not math.isfinite(flow) or level not in (0, 2, 3):
                    raise ValueError(f"invalid low-flow importance sample at {time} s")
                writer.writerow((time, 0.5, format(flow, ".17g"), 1, 1, important, level))
                levels.add(level)
            if levels != ({0, 2, 3} if important else {0}):
                raise ValueError(f"low-flow {name} trace has unexpected alarm levels: {levels}")
            output[VERIFY / f"g36_low_flow_importance_{name}.csv"] = handle.getvalue()
    return output


def main() -> None:
    if sys.argv[1:] not in ([], ["--check"]):
        raise SystemExit("usage: generate_low_flow_importance_modelica_reference.py [--check]")
    for path, content in trends().items():
        if sys.argv[1:] == ["--check"]:
            if path.read_text(encoding="utf-8") != content:
                raise SystemExit(f"{path} is out of date")
        else:
            path.write_text(content, encoding="utf-8", newline="")


if __name__ == "__main__":
    main()
