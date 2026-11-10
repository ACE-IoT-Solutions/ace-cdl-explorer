#!/usr/bin/env python3
"""Pin a positive OBC pressure-request trace against the shipped editable VAV branch."""

import csv
import hashlib
import io
import json
import math
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VERIFY = ROOT / "verification"
EXAMPLE = ROOT / "web/public/examples/four_pipe_g36.json"
RAW = VERIFY / "g36_pressure_requests_positive_openmodelica_raw.csv"
GRAPH = VERIFY / "g36_pressure_requests_positive.json"
TREND = VERIFY / "g36_pressure_requests_positive.csv"
PINNED = {
    "g36_system_requests_modelica.mo": "5fa0c4aa6a2f85ede4b7cf55795229bd4e9d26beaf80f1290608bb639067727e",
    "G36PressureRequestsPositive.mo": "ca02ebee8075d38bb0671c4803eb4bf6b5e1e60c1cbde08786740e1960e054cf",
    "g36_pressure_requests_positive.mos": "2bb091a4f675fbdb3c757d3873c48025d6fad29170939ba2aebb194bbb73e82a",
    "g36_pressure_requests_positive_openmodelica_raw.csv": "a86f90a9ac5b1a3c9d8a2388b7552cccde0a7c24e82387d70fa953e9c4ff648b",
}
STEP = 30
STOP = 3600


def graph_from_shipped_zone() -> dict:
    building = json.loads(EXAMPLE.read_text())
    zone = next(block for block in building["blocks"] if block["name"] == "zone_01")
    needed = {"pressure_importance_select"}
    while True:
        added = {edge["from"].split(".")[0] for edge in zone["connections"]
                 if edge["to"].split(".")[0] in needed
                 and edge["from"].split(".")[0].startswith("pressure_")}
        if added <= needed:
            break
        needed |= added
    inputs = [
        {"name": "damper_position", "type": "Real", "unit": "fraction (0–1)"},
        {"name": "airflow_setpoint", "type": "Real", "unit": "normalized airflow"},
        {"name": "measured_airflow", "type": "Real", "unit": "normalized airflow"},
    ]
    source_map = {
        "heating_damper_select.y": "damper_position",
        "flow_override_select.y": "airflow_setpoint",
        "measured_airflow": "measured_airflow",
    }
    edges = []
    for edge in zone["connections"]:
        target = edge["to"]
        if target.split(".")[0] not in needed and target != "pressure_reset_request":
            continue
        source = edge["from"]
        if source.split(".")[0] not in needed:
            if source not in source_map:
                raise ValueError(f"unexpected external request input {source}")
            source = source_map[source]
        edges.append({"from": source, "to": target})
    return {
        "name": "ShippedVAVPressureRequests",
        "block_type": "Composite",
        "category": "composite",
        "inputs": inputs,
        "outputs": [next(port for port in zone["outputs"]
                         if port["name"] == "pressure_reset_request")],
        "blocks": [block for block in zone["blocks"] if block["name"] in needed],
        "connections": edges,
    }


def trend_from_modelica() -> str:
    for name, expected in PINNED.items():
        if hashlib.sha256((VERIFY / name).read_bytes()).hexdigest() != expected:
            raise ValueError(f"pinned pressure request source changed: {name}")
    samples = {}
    with RAW.open(newline="", encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            time = float(row["time"])
            tick = round(time / STEP)
            if abs(time - tick * STEP) < 1e-3 and 0 <= tick * STEP <= STOP:
                samples[tick * STEP] = row
    if sorted(samples) != list(range(0, STOP + STEP, STEP)):
        raise ValueError("OpenModelica pressure request trace does not cover every 30 s sample")
    with io.StringIO(newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(("time_s", "damper_position", "airflow_setpoint", "measured_airflow", "y"))
        levels = set()
        for time, row in sorted(samples.items()):
            damper = float(row["dam.y"])
            measured = float(row["measuredFlow.y"])
            level = int(row["req.yZonPreResReq"])
            if not math.isfinite(damper) or not math.isfinite(measured) or level not in range(4):
                raise ValueError(f"invalid reference at {time} s")
            writer.writerow((time, format(damper, ".17g"), 1,
                             format(measured, ".17g"), level))
            levels.add(level)
        if levels != set(range(4)):
            raise ValueError(f"positive request trace did not exercise all levels: {levels}")
        return handle.getvalue()


def main() -> None:
    if sys.argv[1:] not in ([], ["--check"]):
        raise SystemExit("usage: generate_pressure_requests_modelica_reference.py [--check]")
    files = {
        GRAPH: json.dumps(graph_from_shipped_zone(), indent=2, ensure_ascii=False) + "\n",
        TREND: trend_from_modelica(),
    }
    for path, content in files.items():
        if sys.argv[1:] == ["--check"]:
            if path.read_text(encoding="utf-8") != content:
                raise SystemExit(f"{path} is out of date")
        else:
            path.write_text(content, encoding="utf-8", newline="")


if __name__ == "__main__":
    main()
