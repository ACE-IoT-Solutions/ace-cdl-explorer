#!/usr/bin/env python3
"""Recreate inputs and expected modes for pinned OBC OperationMode validation.

The Dymola result stores mode, occupancy, next occupancy, and zone temperature.
The validation model's other ports follow its one-zone comparators and hysteresis
with the fixed unoccupied setpoints documented in the pinned validation source.
"""

import argparse
import csv
import hashlib
import io
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "verification/g36_operation_mode_dymola.txt"
SOURCE_SHA256 = "28ca09d88efa4ac08db44fbddb186e7690b47a2b4d89b113c7e41884b954ccb5"
TARGET = ROOT / "verification/g36_operation_mode.csv"
BUILDING_TARGET = ROOT / "verification/g36_building_operation_mode.csv"
HEADER = (
    "time_s", "u1Occ", "tNexOcc", "maxCooDowTim", "u1HigOccCoo",
    "maxWarUpTim", "u1OccHeaHig", "uOpeWin", "totColZon",
    "u1SetBac", "u1EndSetBac", "TZonMin", "totHotZon", "u1SetUp",
    "u1EndSetUp", "y",
)


def render():
    raw = SOURCE.read_bytes()
    if hashlib.sha256(raw).hexdigest() != SOURCE_SHA256:
        raise ValueError("pinned Dymola OperationMode result changed")
    source = raw.decode()
    series = {
        name: [float(value.strip()) for value in values.split(",")]
        for name, values in re.findall(r"(?m)^([^=\n]+)=\[([^\n]*)\]$", source)
    }
    for name in ("opeModSel.yOpeMod", "opeModSel.u1Occ", "opeModSel.u1HigOccCoo",
                 "opeModSel.u1OccHeaHig", "opeModSel.tNexOcc", "opeModSel.TZonMin"):
        if len(series.get(name, [])) != 101:
            raise ValueError(f"{name}: expected 101 pinned Dymola samples")
    for name in ("TZonHeaSetUno.y", "TZonCooSetUno.y", "opeModSel.maxCooDowTim",
                 "opeModSel.maxWarUpTim"):
        if len(series.get(name, [])) != 2 or series[name][0] != series[name][1]:
            raise ValueError(f"{name}: expected a constant pinned Dymola input")
    heat = series["TZonHeaSetUno.y"][0]
    cool = series["TZonCooSetUno.y"][0]
    cool_time = series["opeModSel.maxCooDowTim"][0]
    warm_time = series["opeModSel.maxWarUpTim"][0]
    end_setback = False
    end_setup = False
    handle = io.StringIO(newline="")
    writer = csv.writer(handle, lineterminator="\n")
    writer.writerow(HEADER)
    for index in range(101):
        zone = series["opeModSel.TZonMin"][index]
        end_setback = True if zone - heat > 0.5 else False if zone - heat < -0.5 else end_setback
        end_setup = True if cool - zone > 0.5 else False if cool - zone < -0.5 else end_setup
        cold = int(zone < heat)
        hot = int(zone > cool)
        writer.writerow((
            index * 1728,
            int(series["opeModSel.u1Occ"][index]),
            format(series["opeModSel.tNexOcc"][index], ".17g"),
            format(cool_time, ".17g"),
            int(series["opeModSel.u1HigOccCoo"][index]),
            format(warm_time, ".17g"),
            int(series["opeModSel.u1OccHeaHig"][index]),
            0, cold, cold, int(end_setback), format(zone, ".17g"),
            hot, hot, int(end_setup),
            int(series["opeModSel.yOpeMod"][index]),
        ))
    return handle.getvalue()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    options = parser.parse_args()
    content = render()
    source_rows = list(csv.DictReader(io.StringIO(content)))
    building = io.StringIO(newline="")
    writer = csv.DictWriter(building, fieldnames=HEADER, lineterminator="\n")
    writer.writeheader()
    for row in source_rows:
        for name in ("totColZon", "totHotZon"):
            row[name] = str(5 * int(row[name]))
        row["TZonMin"] = format(float(row["TZonMin"]) - 273.15, ".17g")
        writer.writerow(row)
    building_content = building.getvalue()
    if options.check:
        for target, expected in ((TARGET, content), (BUILDING_TARGET, building_content)):
            if not target.exists() or target.read_text() != expected:
                raise ValueError(f"reference fixture drift: {target}")
    else:
        TARGET.write_text(content)
        BUILDING_TARGET.write_text(building_content)


if __name__ == "__main__":
    main()
