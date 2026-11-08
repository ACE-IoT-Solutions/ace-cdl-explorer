#!/usr/bin/env python3
"""Build replay fixtures from the pinned OBC GroupStatus Dymola validation."""

import argparse
import csv
import hashlib
import io
import json
import re
from pathlib import Path

from obc_group_status import group_status


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "verification/g36_group_status_dymola.txt"
SOURCE_SHA256 = "41e3dc074ac2bfbd5b16f5199b1ecabf6f2691460f226e6623258dca6be95507"
VALIDATION_SOURCE = ROOT / "verification/g36_group_status_validation_modelica.mo"
VALIDATION_SHA256 = "8f318f7299dd159070e6d1ef03d4e03c047faadd95db808481d6adf90febcd22"
GRAPH = ROOT / "verification/g36_group_status_validation.json"
OUTPUTS = ("uGroOcc", "nexOcc", "yCooTim", "yWarTim", "yOccHeaHig",
           "yColZon", "yOpeWin", "ySetBac", "yEndSetBac", "TZonMax", "TZonMin")
BUILDING_OUTPUTS = tuple(output for output in OUTPUTS if output != "ySetBac")
CONSTANTS = {
    "zonOcc": (0, 0), "uCooTim": (1600, 1800),
    "uWarTim": (1800, 1900), "u1HigOccCoo": (0, 0),
    "THeaSetOff": (285.15, 283.15), "u1HigUnoCoo": (0, 0),
    "TCooSetOff": (303.15, 305.15), "u1EndSetUp": (0, 0),
}


def render():
    if hashlib.sha256(SOURCE.read_bytes()).hexdigest() != SOURCE_SHA256:
        raise ValueError("pinned Dymola GroupStatus result changed")
    if hashlib.sha256(VALIDATION_SOURCE.read_bytes()).hexdigest() != VALIDATION_SHA256:
        raise ValueError("pinned GroupStatus validation model changed")
    series = {
        name: [float(value.strip()) for value in values.split(",")]
        for name, values in re.findall(r"(?m)^([^=\n]+)=\[([^\n]*)\]$",
                                       SOURCE.read_text())
    }
    graph = group_status(2, 3)
    header = ["time_s", *(port["name"] for port in graph["inputs"]), "y"]
    rendered = {}
    for output in OUTPUTS:
        expected = series[f"zonGroSta.{output}"]
        if len(expected) not in (2, 101):
            raise ValueError(f"{output}: expected constant or 101 samples")
        handle = io.StringIO(newline="")
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(header)
        for row in range(101):
            values = []
            for port in graph["inputs"]:
                base, zone = port["name"].rsplit("_", 1)
                location = int(zone) - 1
                if base in CONSTANTS:
                    value = CONSTANTS[base][location]
                else:
                    observed = series[f"zonGroSta.{base}[{(1, 3)[location]}]"]
                    if len(observed) != 101:
                        raise ValueError(f"{base}: expected 101 samples")
                    value = observed[row]
                values.append(format(value, ".17g"))
            writer.writerow([row * 36, *values,
                             format(expected[row] if len(expected) == 101 else expected[0],
                                    ".17g")])
        rendered[output] = handle.getvalue()
    building = group_status(5)
    building_header = ["time_s", *(port["name"] for port in building["inputs"]), "y"]
    building_rendered = {}
    for output in BUILDING_OUTPUTS:
        source_rows = list(csv.DictReader(io.StringIO(rendered[output])))
        handle = io.StringIO(newline="")
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(building_header)
        for original in source_rows:
            values = []
            for port in building["inputs"]:
                base, zone = port["name"].rsplit("_", 1)
                zone = int(zone)
                if zone <= 2:
                    value = float(original[port["name"]])
                elif base == "tNexOcc":
                    value = 1e9
                elif base in ("u1EndSetBac", "u1EndSetUp", "u1Win"):
                    value = 1
                elif base in ("THeaSetOff", "TCooSetOff", "TZon"):
                    value = float(original[f"{base}_1"])
                else:
                    value = 0
                if base in ("THeaSetOff", "TCooSetOff", "TZon"):
                    value -= 273.15
                values.append(format(value, ".17g"))
            expected = float(original["y"])
            if output in ("TZonMax", "TZonMin"):
                expected -= 273.15
            writer.writerow([original["time_s"], *values, format(expected, ".17g")])
        building_rendered[output] = handle.getvalue()
    return json.dumps(graph, indent=2) + "\n", rendered, building_rendered


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    options = parser.parse_args()
    graph, trends, building_trends = render()
    files = {GRAPH: graph}
    files.update({ROOT / f"verification/g36_group_status_{output}.csv": content
                  for output, content in trends.items()})
    files.update({ROOT / f"verification/g36_building_group_status_{output}.csv": content
                  for output, content in building_trends.items()})
    for path, content in files.items():
        if options.check:
            if not path.exists() or path.read_text() != content:
                raise ValueError(f"GroupStatus fixture drift: {path}")
        else:
            path.write_text(content)


if __name__ == "__main__":
    main()
