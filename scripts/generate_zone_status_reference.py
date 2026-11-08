#!/usr/bin/env python3
"""Extract OBC G36 ZoneStatus trends from a pinned Dymola validation result.

Source: modelica-buildings d3d8e6f76514dfbc7d65574c6b5a777b686fe1b7,
Buildings/Resources/ReferenceResults/Dymola/
Buildings_Controls_OBC_ASHRAE_G36_ZoneGroups_Validation_ZoneStatus.txt.
"""

import argparse
import csv
import io
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "verification/g36_zone_status_dymola.txt"
OUTPUTS = ("yOccHeaHig", "yHigOccCoo", "yUnoHeaHig", "yEndSetBac")
HEADER = (
    "time_s", "cooDowTim", "warUpTim", "TZon", "TOccHeaSet",
    "TOccCooSet", "TUnoHeaSet", "TUnoCooSet", "u1Win", "y",
)


def read_series() -> dict[str, list[float]]:
    series = {}
    for name, values in re.findall(r"(?m)^([^=\n]+)=\[([^\n]*)\]$", SOURCE.read_text()):
        series[name] = [float(value.strip()) for value in values.split(",")]
    required = ["noWinZonSta.TZon", "witWinZonSta.u1Win"]
    constants = ("THeaSetOcc.y", "TCooSetOcc.y", "THeaSetUno.y", "TCooSetUno.y")
    for name in constants:
        if len(series.get(name, [])) != 2 or series[name][0] != series[name][1]:
            raise ValueError(f"{name}: expected a constant pinned Dymola input")
    required += [f"{variant}.{output}" for variant in ("noWinZonSta", "witWinZonSta") for output in OUTPUTS]
    for name in required:
        if len(series.get(name, [])) != 101:
            raise ValueError(f"{name}: expected 101 pinned Dymola samples")
    return series


def render(series: dict[str, list[float]], variant: str, output: str) -> str:
    handle = io.StringIO(newline="")
    writer = csv.writer(handle, lineterminator="\n")
    writer.writerow(HEADER)
    for index, (zone_temp, expected) in enumerate(zip(series["noWinZonSta.TZon"], series[f"{variant}.{output}"])):
        window_closed = 1 if variant == "noWinZonSta" else int(series["witWinZonSta.u1Win"][index])
        writer.writerow((
            index * 864, 1800, 1800, format(zone_temp, ".17g"),
            *(format(series[name][0], ".17g") for name in (
                "THeaSetOcc.y", "TCooSetOcc.y", "THeaSetUno.y", "TCooSetUno.y"
            )),
            window_closed, int(expected),
        ))
    return handle.getvalue()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="fail if checked-in trends differ")
    options = parser.parse_args()
    series = read_series()
    for variant, label in (("noWinZonSta", "no_window"), ("witWinZonSta", "with_window")):
        for output in OUTPUTS:
            target = ROOT / f"verification/g36_zone_status_{label}_{output}.csv"
            content = render(series, variant, output)
            if options.check:
                if not target.exists() or target.read_text() != content:
                    raise ValueError(f"reference fixture drift: {target}")
            else:
                target.write_text(content)


if __name__ == "__main__":
    main()
