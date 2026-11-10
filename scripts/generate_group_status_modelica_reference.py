#!/usr/bin/env python3
"""Extract all OBC GroupStatus outputs from the pinned OpenModelica validation."""

import csv
import hashlib
import io
import math
import sys
from pathlib import Path

from obc_group_status import group_status


ROOT = Path(__file__).resolve().parents[1]
VERIFY = ROOT / "verification"
PINNED = {
    "g36_group_status_modelica.mo": "246e73aca6f39a150ab95603cba43fca8d022820f0d6cf7549ed64d446626160",
    "g36_group_status_validation_modelica.mo": "8f318f7299dd159070e6d1ef03d4e03c047faadd95db808481d6adf90febcd22",
    "g36_group_status_openmodelica_raw.csv": "7e52087e7b9595acb1f368474ba4fb5687b8dbcccf66f3063658ff1a1a1dd444",
}
OUTPUTS = tuple(port["name"] for port in group_status(2, 3)["outputs"])
STEP = 36
STOP = 3600


def read_samples() -> dict[int, dict[str, str]]:
    for name, expected in PINNED.items():
        actual = hashlib.sha256((VERIFY / name).read_bytes()).hexdigest()
        if actual != expected:
            raise ValueError(f"{name}: pinned Modelica reference changed")
    rows: dict[int, dict[str, str]] = {}
    with (VERIFY / "g36_group_status_openmodelica_raw.csv").open(
        newline="", encoding="utf-8"
    ) as handle:
        for row in csv.DictReader(handle):
            time = float(row["time"])
            tick = round(time / STEP)
            if abs(time - tick * STEP) <= 1e-3 and 0 <= tick * STEP <= STOP:
                rows[tick * STEP] = row
    if sorted(rows) != list(range(0, STOP + STEP, STEP)):
        raise ValueError("OpenModelica GroupStatus trace does not cover all 101 samples")
    return rows


def standalone(rows: dict[int, dict[str, str]], output: str) -> str:
    graph = group_status(2, 3)
    with io.StringIO(newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(("time_s", *(port["name"] for port in graph["inputs"]), "y"))
        for time in range(0, STOP + STEP, STEP):
            row = rows[time]
            values = []
            for port in graph["inputs"]:
                base, zone = port["name"].rsplit("_", 1)
                value = row[f"zonGroSta.{base}[{(1, 3)[int(zone) - 1]}]"]
                if not math.isfinite(float(value)):
                    raise ValueError(f"nonfinite GroupStatus input {port['name']} at {time} s")
                values.append(value)
            result = row[f"zonGroSta.{output}"]
            if not math.isfinite(float(result)):
                raise ValueError(f"nonfinite GroupStatus output {output} at {time} s")
            writer.writerow((time, *values, result))
        return handle.getvalue()


def building(standalone_trend: str, output: str) -> str:
    graph = group_status(5)
    with io.StringIO(newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(("time_s", *(port["name"] for port in graph["inputs"]), "y"))
        for row in csv.DictReader(io.StringIO(standalone_trend)):
            values = []
            for port in graph["inputs"]:
                base, zone = port["name"].rsplit("_", 1)
                zone = int(zone)
                if zone <= 2:
                    value = float(row[port["name"]])
                elif base == "tNexOcc":
                    value = 1e9
                elif base in ("u1EndSetBac", "u1EndSetUp", "u1Win"):
                    value = 1.0
                elif base in ("THeaSetOff", "TCooSetOff", "TZon"):
                    value = float(row[f"{base}_1"])
                else:
                    value = 0.0
                if base in ("THeaSetOff", "TCooSetOff", "TZon"):
                    value -= 273.15
                values.append(format(value, ".17g"))
            result = float(row["y"])
            if output in ("TZonMax", "TZonMin"):
                result -= 273.15
            writer.writerow((row["time_s"], *values, format(result, ".17g")))
        return handle.getvalue()


def main() -> None:
    if sys.argv[1:] not in ([], ["--check"]):
        raise SystemExit("usage: generate_group_status_modelica_reference.py [--check]")
    rows = read_samples()
    for output in OUTPUTS:
        standalone_trend = standalone(rows, output)
        files = {
            VERIFY / f"g36_group_status_modelica_{output}.csv": standalone_trend,
            VERIFY / f"g36_building_group_status_modelica_{output}.csv":
                building(standalone_trend, output),
        }
        for target, content in files.items():
            if sys.argv[1:] == ["--check"]:
                if target.read_text(encoding="utf-8") != content:
                    raise SystemExit(f"{target} is out of date")
            else:
                target.write_text(content, encoding="utf-8", newline="")


if __name__ == "__main__":
    main()
