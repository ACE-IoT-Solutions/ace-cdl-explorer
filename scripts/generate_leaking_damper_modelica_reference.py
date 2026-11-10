#!/usr/bin/env python3
"""Extract an active OBC leaking-damper trace for the shipped CDL alarm branch."""

import csv
import hashlib
import io
import math
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VERIFY = ROOT / "verification"
RAW = VERIFY / "g36_leaking_damper_positive_openmodelica_raw.csv"
TREND = VERIFY / "g36_leaking_damper_positive.csv"
PINNED = {
    "g36_reheat_alarms_modelica.mo": "4ea2e6e267916e0759423f76c578b01487da6f55d9d0ee5fc18a101e102e96ce",
    "G36LeakingDamperPositive.mo": "ce40d97af7875e6cf1c901d88d3e4eff16a06e7568d6f03164f7568d0d403045",
    "g36_leaking_damper_positive.mos": "e54bdccb79052126612bd6f6810b9ee7133e4649eb4c7a33468eac8ce8a9669d",
    "g36_leaking_damper_positive_openmodelica_raw.csv": "31a2f7dc7f46f57ecb910734899361400419bd481240b196edc5141f6823716b",
}
STEP = 30
STOP = 3600


def trend() -> str:
    for name, expected in PINNED.items():
        if hashlib.sha256((VERIFY / name).read_bytes()).hexdigest() != expected:
            raise ValueError(f"pinned leaking-damper reference changed: {name}")
    samples = {}
    with RAW.open(newline="", encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            time = float(row["time"])
            tick = round(time / STEP)
            if abs(time - tick * STEP) < 1e-3 and 0 <= tick * STEP <= STOP:
                samples[tick * STEP] = row
    if sorted(samples) != list(range(0, STOP + STEP, STEP)):
        raise ValueError("OpenModelica leaking-damper trace does not cover 121 samples")
    with io.StringIO(newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(("time_s", "damper_position", "fan_proven", "measured_flow", "y"))
        levels = set()
        rising = falling = False
        previous = 0
        for time, row in sorted(samples.items()):
            flow = float(row["measuredFlow.y"])
            level = int(row["ala.yLeaDamAla"])
            if not math.isfinite(flow) or level not in (0, 4):
                raise ValueError(f"invalid leaking-damper sample at {time} s")
            writer.writerow((time, 0, 1, format(flow, ".17g"), level))
            levels.add(level)
            rising |= previous == 0 and level == 4
            falling |= previous == 4 and level == 0
            previous = level
        if levels != {0, 4} or not rising or not falling:
            raise ValueError("leaking-damper reference must assert and clear Level 4")
        return handle.getvalue()


def main() -> None:
    if sys.argv[1:] not in ([], ["--check"]):
        raise SystemExit("usage: generate_leaking_damper_modelica_reference.py [--check]")
    content = trend()
    if sys.argv[1:] == ["--check"]:
        if TREND.read_text(encoding="utf-8") != content:
            raise SystemExit(f"{TREND} is out of date")
    else:
        TREND.write_text(content, encoding="utf-8", newline="")


if __name__ == "__main__":
    main()
