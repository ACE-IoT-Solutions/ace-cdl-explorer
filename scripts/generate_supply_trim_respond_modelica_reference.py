#!/usr/bin/env python3
"""Compare the shipped AHU SAT trim branch with pinned OBC TrimAndRespond."""

import csv
import hashlib
import io
import json
import math
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VERIFY = ROOT / "verification"
RAW = VERIFY / "g36_supply_trim_respond_openmodelica_raw.csv"
GRAPH = VERIFY / "g36_supply_trim_respond_graph.json"
TREND = VERIFY / "g36_supply_trim_respond.csv"
PINNED = {
    "g36_trim_respond_modelica.mo": "10fbf404cbfba7b762b65095014d3def871dbd6fa1763794e5999397c5486df4",
    "G36SupplyTrimRespond.mo": "d7a5928551b930bee77cee7317e20fb02edde1001aa61b24a2e07b1dc9374fa2",
    "g36_supply_trim_respond.mos": "79b1af6fb3f9769ef7037daef3a2532eb95934642e8f19a203e16418f56ca924",
    "g36_supply_trim_respond_openmodelica_raw.csv": "2db4a049beedd5bbb8035a864f256c5cc69ad27ba5e9d33102e05ff12dd123b4",
}
NAMES = {
    "sat_request_real", "sat_request_sample", "sat_ignored_requests", "sat_excess_requests",
    "sat_zero_requests", "sat_positive_requests", "sat_response_gain", "sat_max_response",
    "sat_bounded_response", "sat_trim", "sat_delta", "sat_target_sum", "sat_previous",
    "sat_minimum", "sat_above_minimum", "sat_below_weather_max", "sat_fan_delay",
    "sat_candidate", "sat_output", "sat_maximum",
}
STEP = 30
STOP = 7200


def graph() -> str:
    building = json.loads((ROOT / "web/public/examples/four_pipe_g36.json").read_text())
    ahu = next(block for block in building["blocks"] if block["name"] == "ahu_1")
    blocks = [block for block in ahu["blocks"] if block["name"] in NAMES]
    if len(blocks) != len(NAMES):
        raise ValueError("shipped AHU SAT trim branch changed; review extraction")
    external = {"sat_request_sum_4.y": "numOfReq", "supply_fan_proven": "fan"}
    edges = []
    for edge in ahu["connections"]:
        if edge["to"].split(".")[0] not in NAMES:
            continue
        source = edge["from"]
        if source.split(".")[0] not in NAMES:
            if source not in external:
                raise ValueError(f"unmapped AHU trim source: {source}")
            source = external[source]
        edges.append({"from": source, "to": edge["to"]})
    edges.append({"from": "sat_output.y", "to": "y"})
    return json.dumps({
        "name": "ShippedG36AhuSupplyTrimRespond",
        "block_type": "Composite", "category": "composite",
        "inputs": [{"name": "numOfReq", "type": "Integer", "unit": "count"},
                   {"name": "fan", "type": "Boolean"}],
        "outputs": [{"name": "y", "type": "Real", "unit": "°C"}],
        "blocks": blocks, "connections": edges,
    }, indent=2) + "\n"


def trend() -> str:
    for name, expected in PINNED.items():
        if hashlib.sha256((VERIFY / name).read_bytes()).hexdigest() != expected:
            raise ValueError(f"pinned supply trim reference changed: {name}")
    samples = {}
    with RAW.open(newline="", encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            time = float(row["time"])
            tick = round(time / STEP)
            if abs(time - tick * STEP) < 1e-3 and 0 <= tick * STEP <= STOP:
                samples[tick * STEP] = row
    if sorted(samples) != list(range(0, STOP + STEP, STEP)):
        raise ValueError("OpenModelica AHU trim trace does not cover 241 samples")
    with io.StringIO(newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(("time_s", "numOfReq", "fan", "y"))
        values = set()
        for time, row in sorted(samples.items()):
            value = float(row["reset.y"])
            if not math.isfinite(value) or not 12 <= value <= 18:
                raise ValueError(f"invalid OBC SAT trim value at {time} s")
            writer.writerow((time, int(row["requestCount.y"]), int(row["fan.y"]),
                             format(value, ".17g")))
            values.add(value)
        if len(values) < 5 or min(values) >= 16:
            raise ValueError("OBC SAT trim trace does not exercise a response")
        return handle.getvalue()


def main() -> None:
    if sys.argv[1:] not in ([], ["--check"]):
        raise SystemExit("usage: generate_supply_trim_respond_modelica_reference.py [--check]")
    for path, content in ((GRAPH, graph()), (TREND, trend())):
        if sys.argv[1:] == ["--check"]:
            if path.read_text(encoding="utf-8") != content:
                raise SystemExit(f"{path} is out of date")
        else:
            path.write_text(content, encoding="utf-8", newline="")


if __name__ == "__main__":
    main()
