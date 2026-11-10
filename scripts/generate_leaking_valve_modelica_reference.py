#!/usr/bin/env python3
"""Extract an active OBC reheat-valve leak trace for the shipped CDL alarm branch."""

import csv
import hashlib
import io
import math
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VERIFY = ROOT / "verification"
RAW = VERIFY / "g36_leaking_valve_positive_openmodelica_raw.csv"
TREND = VERIFY / "g36_leaking_valve_positive.csv"
PINNED = {
    "g36_reheat_alarms_modelica.mo": "4ea2e6e267916e0759423f76c578b01487da6f55d9d0ee5fc18a101e102e96ce",
    "G36LeakingValvePositive.mo": "be5a1b9f43db19ec759953e49abc5e6a833563cb9050e305e29f895a9416fe1b",
    "g36_leaking_valve_positive.mos": "37aaf8f566b8aca2a319c018451c0f525730bb6272b095e3b5dea9f220489b45",
    "g36_leaking_valve_positive_openmodelica_raw.csv": "294e5c4f9d554f1dc8bd4198b4700de0a1b0fcf1c88a811258776d2a8c920925",
}
STEP = 30
STOP = 3600


def trend() -> str:
    for name, expected in PINNED.items():
        if hashlib.sha256((VERIFY / name).read_bytes()).hexdigest() != expected:
            raise ValueError(f"pinned leaking-valve reference changed: {name}")
    samples = {}
    with RAW.open(newline="", encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            time = float(row["time"])
            tick = round(time / STEP)
            if abs(time - tick * STEP) < 1e-3 and 0 <= tick * STEP <= STOP:
                samples[tick * STEP] = row
    if sorted(samples) != list(range(0, STOP + STEP, STEP)):
        raise ValueError("OpenModelica leaking-valve trace does not cover 121 samples")
    with io.StringIO(newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(("time_s", "valve_position", "discharge_c", "supply_c", "fan_proven", "y"))
        levels = set()
        rising = falling = False
        previous = 0
        for time, row in sorted(samples.items()):
            discharge = float(row["discharge.y"]) - 273.15
            level = int(row["ala.yLeaValAla"])
            if not math.isfinite(discharge) or level not in (0, 4):
                raise ValueError(f"invalid leaking-valve sample at {time} s")
            writer.writerow((time, 0, format(discharge, ".17g"), 15, 1, level))
            levels.add(level)
            rising |= previous == 0 and level == 4
            falling |= previous == 4 and level == 0
            previous = level
        if levels != {0, 4} or not rising or not falling:
            raise ValueError("leaking-valve reference must assert and clear Level 4")
        return handle.getvalue()


def main() -> None:
    if sys.argv[1:] not in ([], ["--check"]):
        raise SystemExit("usage: generate_leaking_valve_modelica_reference.py [--check]")
    content = trend()
    if sys.argv[1:] == ["--check"]:
        if TREND.read_text(encoding="utf-8") != content:
            raise SystemExit(f"{TREND} is out of date")
    else:
        TREND.write_text(content, encoding="utf-8", newline="")


if __name__ == "__main__":
    main()
