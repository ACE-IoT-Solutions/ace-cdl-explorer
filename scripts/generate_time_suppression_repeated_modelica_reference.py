#!/usr/bin/env python3
"""Extract repeated-setpoint OBC traces from the shipped VAV CDL branch."""

import csv
import hashlib
import io
import json
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VERIFY = ROOT / "verification"
PINNED = {
    "g36_time_suppression_modelica.mo": "c1851f5a7216567737a4164de43530ef1b9de2ea0a17bb9654e872c309f4d27e",
    "G36TimeSuppressionRepeated.mo": "ab576e03da024f23880ec76e55e3f2cf966996bb1bd00095553482fac10130b3",
    "g36_time_suppression_repeated.mos": "f8a505916aef5d5c0cee1a7acfb3cbf4d8dbb66698496c1d983a2ab3a2406877",
    "g36_time_suppression_repeated_openmodelica_raw.csv": "58c77d6e33e815031c782ff200d8994a99c0e7136c4ed78512e81764d0835f05",
}


def graph() -> str:
    building = json.loads((ROOT / "web/public/examples/four_pipe_g36.json").read_text())
    zone = next(block for block in building["blocks"] if block["name"] == "zone_01")
    blocks = [block for block in zone["blocks"] if block["name"].startswith("sat_")]
    names = {block["name"] for block in blocks}
    if "sat_suppression_complete" not in names or "sat_suppression_duration" not in names:
        raise ValueError("shipped VAV suppression branch changed")
    external = {"window_cool_target.y": "cool_setpoint", "cool_pi.y": "cooling_loop",
                "room_temp": "room_temp"}
    edges = []
    for edge in zone["connections"]:
        if edge["to"].split(".")[0] not in names:
            continue
        source = edge["from"]
        if source.split(".")[0] not in names:
            if source not in external:
                raise ValueError(f"unmapped suppression source: {source}")
            source = external[source]
        edges.append({"from": source, "to": edge["to"]})
    edges.extend([{"from": "sat_suppression_complete.y", "to": "after"},
                  {"from": "sat_suppression_duration.y", "to": "duration"}])
    return json.dumps({"name": "ShippedG36VavTimeSuppression",
                       "block_type": "Composite", "category": "composite",
                       "inputs": [{"name": "room_temp", "type": "Real", "unit": "°C"},
                                  {"name": "cool_setpoint", "type": "Real", "unit": "°C"},
                                  {"name": "cooling_loop", "type": "Real"}],
                       "outputs": [{"name": "after", "type": "Boolean"},
                                   {"name": "duration", "type": "Real", "unit": "s"}],
                       "blocks": blocks, "connections": edges}, indent=2) + "\n"


def trends() -> dict[str, str]:
    for name, expected in PINNED.items():
        if hashlib.sha256((VERIFY / name).read_bytes()).hexdigest() != expected:
            raise ValueError(f"pinned suppression reference changed: {name}")
    samples = {}
    with (VERIFY / "g36_time_suppression_repeated_openmodelica_raw.csv").open(newline="") as handle:
        for row in csv.DictReader(handle):
            time = float(row["time"])
            tick = round(time / 30)
            if abs(time - tick * 30) < 1e-3 and 0 <= tick <= 120:
                samples[tick * 30] = row
    if sorted(samples) != list(range(0, 3601, 30)):
        raise ValueError("OpenModelica repeated suppression trace lacks 121 samples")
    results = {}
    for output, source in (("after", "timSup.yAftSup"), ("duration", "timSup.supTim.y")):
        with io.StringIO(newline="") as handle:
            writer = csv.writer(handle, lineterminator="\n")
            writer.writerow(("time_s", "room_temp", "cool_setpoint", "cooling_loop", "y"))
            values = set()
            for time, row in sorted(samples.items()):
                value = float(row[source])
                if not math.isfinite(value):
                    raise ValueError(f"nonfinite {source} at {time}")
                values.add(value)
                writer.writerow((time, 25, format(float(row["setpoint.y"]) - 273.15, ".17g"),
                                 0, format(value, ".17g")))
            if len(values) < 2:
                raise ValueError(f"reference {source} did not vary")
            results[output] = handle.getvalue()
    return results


def main() -> None:
    if sys.argv[1:] not in ([], ["--check"]):
        raise SystemExit("usage: generate_time_suppression_repeated_modelica_reference.py [--check]")
    contents = {VERIFY / "g36_time_suppression_repeated_graph.json": graph()}
    contents.update({VERIFY / f"g36_time_suppression_repeated_{output}.csv": content
                     for output, content in trends().items()})
    for path, content in contents.items():
        if sys.argv[1:] == ["--check"]:
            if path.read_text(encoding="utf-8") != content:
                raise SystemExit(f"{path} is out of date")
        else:
            path.write_text(content, encoding="utf-8", newline="")


if __name__ == "__main__":
    main()
