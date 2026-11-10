#!/usr/bin/env python3
"""Extract every OBC ZoneStatus output from the pinned OpenModelica validation."""

import csv
import hashlib
import io
import math
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VERIFY = ROOT / "verification"
PINNED = {
    "g36_zone_status_modelica.mo": "7c5f1ba64608f07a27be3ebe20afc2b04ce4f9f3c643d29cb7588f3fc46abbaf",
    "g36_zone_status_validation_modelica.mo": "044d29df6d54c3559b43b757f8534046b2fb4818c2135c75fa923999d77d7620",
    "g36_zone_status_openmodelica_raw.csv": "8778454ccfa5fdbaba86a5c470695470117dc54e4f6781271e9a281a75db10a6",
}
OUTPUTS = ("yCooTim", "yWarTim", "yOccHeaHig", "yHigOccCoo", "yUnoHeaHig",
           "yEndSetBac", "yHigUnoCoo", "yEndSetUp")
BOOLEAN_OUTPUTS = set(OUTPUTS[2:])
HEADER = ("time_s", "cooDowTim", "warUpTim", "TZon", "TOccHeaSet",
          "TOccCooSet", "TUnoHeaSet", "TUnoCooSet", "u1Win", "y")
STEP = 864
STOP = 86400


def samples() -> dict[int, dict[str, str]]:
    for name, expected in PINNED.items():
        actual = hashlib.sha256((VERIFY / name).read_bytes()).hexdigest()
        if actual != expected:
            raise ValueError(f"{name}: pinned Modelica reference changed")
    rows: dict[int, dict[str, str]] = {}
    with (VERIFY / "g36_zone_status_openmodelica_raw.csv").open(
        newline="", encoding="utf-8"
    ) as handle:
        for row in csv.DictReader(handle):
            time = float(row["time"])
            tick = round(time / STEP)
            if abs(time - tick * STEP) <= 1e-3 and 0 <= tick * STEP <= STOP:
                rows[tick * STEP] = row
    if sorted(rows) != list(range(0, STOP + STEP, STEP)):
        raise ValueError("OpenModelica ZoneStatus trace does not cover the 101 sample times")
    if {row["witWinZonSta.u1Win"] for row in rows.values()} != {"0", "1"}:
        raise ValueError("ZoneStatus validation does not exercise both window states")
    for variant in ("noWinZonSta", "witWinZonSta"):
        for output in BOOLEAN_OUTPUTS:
            if {row[f"{variant}.{output}"] for row in rows.values()} != {"0", "1"}:
                raise ValueError(f"ZoneStatus validation does not exercise both {variant}.{output} states")
    for output in ("yCooTim", "yWarTim"):
        if {row[f"witWinZonSta.{output}"] for row in rows.values()} != {"0", "1800"}:
            raise ValueError(f"ZoneStatus validation does not exercise window gating for {output}")
    return rows


def render(rows: dict[int, dict[str, str]], variant: str, output: str) -> str:
    prefix = "noWinZonSta" if variant == "no_window" else "witWinZonSta"
    with io.StringIO(newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(HEADER)
        for time in range(0, STOP + STEP, STEP):
            row = rows[time]
            fields = (row["cooDowTim.y"], row["warUpTim.y"], row["zonTem.y"],
                      row["THeaSetOcc.y"], row["TCooSetOcc.y"],
                      row["THeaSetUno.y"], row["TCooSetUno.y"])
            if not all(math.isfinite(float(value)) for value in fields):
                raise ValueError(f"ZoneStatus has nonfinite input at {time} s")
            window = "1" if variant == "no_window" else row["witWinZonSta.u1Win"]
            value = row[f"{prefix}.{output}"]
            if window not in ("0", "1") or (output in BOOLEAN_OUTPUTS and value not in ("0", "1")):
                raise ValueError(f"ZoneStatus has invalid Boolean value at {time} s")
            if not math.isfinite(float(value)):
                raise ValueError(f"ZoneStatus has nonfinite output at {time} s")
            writer.writerow((time, *fields, window, value))
        return handle.getvalue()


def main() -> None:
    if sys.argv[1:] not in ([], ["--check"]):
        raise SystemExit("usage: generate_zone_status_modelica_reference.py [--check]")
    rows = samples()
    for variant in ("no_window", "with_window"):
        for output in OUTPUTS:
            target = VERIFY / f"g36_zone_status_modelica_{variant}_{output}.csv"
            content = render(rows, variant, output)
            if sys.argv[1:] == ["--check"]:
                if target.read_text(encoding="utf-8") != content:
                    raise SystemExit(f"{target} is out of date")
            else:
                target.write_text(content, encoding="utf-8", newline="")


if __name__ == "__main__":
    main()
