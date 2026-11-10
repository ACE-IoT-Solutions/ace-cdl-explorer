#!/usr/bin/env python3
"""Replay the archived OBC reheat alarm trace through the shipped CDL branch."""

import argparse
import csv
import hashlib
import io
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ARCHIVE = ROOT / "verification/g36_reheat_alarms_dymola.txt"
GRAPH = ROOT / "verification/g36_low_discharge_alarm_graph.json"
TREND = ROOT / "verification/g36_low_discharge_alarm.csv"
FLOW_GRAPH = ROOT / "verification/g36_low_flow_alarm_graph.json"
FLOW_TREND = ROOT / "verification/g36_low_flow_alarm.csv"
SENSOR_GRAPH = ROOT / "verification/g36_airflow_sensor_alarm_graph.json"
SENSOR_TREND = ROOT / "verification/g36_airflow_sensor_alarm.csv"
VALVE_GRAPH = ROOT / "verification/g36_leaking_valve_alarm_graph.json"
VALVE_TREND = ROOT / "verification/g36_leaking_valve_alarm.csv"
DAMPER_GRAPH = ROOT / "verification/g36_leaking_damper_alarm_graph.json"
PINNED_SHA256 = "337818ba51b20773883ae888b850a16460e4a7600cb0be4b4e944278966230ce"
EXTERNAL = {
    "supply_fan_proven": "fan_proven",
    "hot_water_plant_proven": "hot_plant_proven",
    "local_occupied.y": "occupied",
    "discharge_target.y": "setpoint_c",
    "discharge_temp": "measured_c",
}


def source_arrays():
    data = ARCHIVE.read_bytes()
    if hashlib.sha256(data).hexdigest() != PINNED_SHA256:
        raise ValueError("pinned OBC reheat alarm validation trace changed")
    arrays = {}
    for line in data.decode().splitlines():
        if line.startswith("ala."):
            name, raw = line.split("=", 1)
            arrays[name] = [float(value) for value in raw.strip("[]").split(",")]
    for name in ("ala.u1Fan", "ala.uOpeMod", "ala.TDis", "ala.yLowTemAla"):
        if len(arrays[name]) != 101:
            raise ValueError(f"{name} needs 101 archived samples")
    return arrays


def alarm_graph(prefixes=("low_discharge_",), output="low_discharge_temperature_alarm_level",
                external=EXTERNAL, expected_blocks=20):
    building = json.loads((ROOT / "web/public/examples/four_pipe_g36.json").read_text())
    zone = next(block for block in building["blocks"] if block["name"] == "zone_01")
    blocks = [block for block in zone["blocks"] if block["name"].startswith(prefixes)]
    names = {block["name"] for block in blocks}
    if len(blocks) != expected_blocks:
        raise ValueError(f"shipped {output} CDL branch changed; review extraction")
    edges = []
    for edge in zone["connections"]:
        target = edge["to"]
        if target.split(".")[0] not in names and target != output:
            continue
        source = edge["from"]
        if source.split(".")[0] not in names:
            if source not in external:
                raise ValueError(f"unmapped {output} source: {source}")
            source = external[source]
        edges.append({"from": source, "to": target})
    input_types = {"fan_proven": "Boolean", "hot_plant_proven": "Boolean",
                   "occupied": "Boolean", "important": "Boolean",
                   "setpoint_c": "Real", "measured_c": "Real",
                   "setpoint_flow": "Real", "measured_flow": "Real",
                   "valve_position": "Real", "damper_position": "Real",
                   "discharge_c": "Real", "supply_c": "Real"}
    inputs = list(dict.fromkeys(external.values()))
    return {
        "name": "ShippedG36" + "".join(part.title() for part in output.split("_")),
        "block_type": "Composite",
        "category": "composite",
        "inputs": [{"name": name, "type": input_types[name],
                    **({"unit": "°C"} if name.endswith("_c") else {})} for name in inputs],
        "outputs": [next(port for port in zone["outputs"]
                         if port["name"] == output)],
        "blocks": blocks,
        "connections": edges,
    }


def alarm_trend(arrays):
    output = io.StringIO()
    writer = csv.writer(output, lineterminator="\n")
    writer.writerow(["time_s", "fan_proven", "hot_plant_proven", "occupied",
                     "setpoint_c", "measured_c", "y"])
    for index in range(101):
        seconds = index * 72
        writer.writerow([seconds, int(arrays["ala.u1Fan"][index] != 0),
                         int(seconds < 6750), int(arrays["ala.uOpeMod"][index] == 1),
                         arrays["ala.TDisSet"][0] - 273.15,
                         arrays["ala.TDis"][index] - 273.15,
                         int(arrays["ala.yLowTemAla"][index])])
    return output.getvalue()


def flow_trend(arrays, expected):
    output = io.StringIO()
    writer = csv.writer(output, lineterminator="\n")
    writer.writerow(["time_s", "setpoint_flow", "measured_flow", "fan_proven",
                     "occupied", "important", "y"] if expected == "ala.yLowFloAla" else
                    ["time_s", "fan_proven", "measured_flow", "y"])
    for index in range(101):
        seconds = index * 72
        fan = int(arrays["ala.u1Fan"][index] != 0)
        measured = arrays["ala.VDis_flow"][index]
        if expected == "ala.yLowFloAla":
            writer.writerow([seconds, arrays["ala.VActSet_flow"][index], measured, fan,
                             int(arrays["ala.uOpeMod"][index] == 1), 1,
                             int(arrays[expected][index])])
        else:
            writer.writerow([seconds, fan, measured, int(arrays[expected][index])])
    return output.getvalue()


def valve_trend(arrays):
    output = io.StringIO()
    writer = csv.writer(output, lineterminator="\n")
    writer.writerow(["time_s", "valve_position", "discharge_c", "supply_c", "fan_proven", "y"])
    for index in range(101):
        writer.writerow([index * 72, arrays["ala.uVal"][index],
                         arrays["ala.TDis"][index] - 273.15,
                         arrays["ala.TSup"][0] - 273.15,
                         int(arrays["ala.u1Fan"][index] != 0),
                         int(arrays["ala.yLeaValAla"][index])])
    return output.getvalue()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    outputs = {
        GRAPH: json.dumps(alarm_graph(), indent=2) + "\n",
        TREND: alarm_trend(source_arrays()),
        FLOW_GRAPH: json.dumps(alarm_graph(
            ("low_flow_", "flow_requested", "flow_alarm_enable_threshold"),
            "low_airflow_alarm_level", {
                "flow_override_select.y": "setpoint_flow",
                "measured_airflow": "measured_flow",
                "supply_fan_proven": "fan_proven",
                "local_occupied.y": "occupied",
                "pressure_importance_positive.y": "important",
            }, 24), indent=2) + "\n",
        FLOW_TREND: flow_trend(source_arrays(), "ala.yLowFloAla"),
        SENSOR_GRAPH: json.dumps(alarm_graph(
            ("sensor_",), "airflow_sensor_alarm_level", {
                "supply_fan_proven": "fan_proven",
                "measured_airflow": "measured_flow",
            }, 9), indent=2) + "\n",
        SENSOR_TREND: flow_trend(source_arrays(), "ala.yFloSenAla"),
        VALVE_GRAPH: json.dumps(alarm_graph(
            ("leak_valve_", "leak_alarm_"), "leaking_reheat_valve_alarm_level", {
                "actual_reheat_valve_position": "valve_position",
                "discharge_temp": "discharge_c",
                "supply_air_temp": "supply_c",
                "supply_fan_proven": "fan_proven",
            }, 11), indent=2) + "\n",
        VALVE_TREND: valve_trend(source_arrays()),
        DAMPER_GRAPH: json.dumps(alarm_graph(
            ("leak_damper_", "leak_alarm_", "sensor_ten_percent", "sensor_cooling_design_flow"),
            "leaking_damper_alarm_level", {
                "damper_feedback_output.y": "damper_position",
                "supply_fan_proven": "fan_proven",
                "measured_airflow": "measured_flow",
            }, 11), indent=2) + "\n",
    }
    for path, value in outputs.items():
        if args.check:
            if not path.exists() or path.read_text() != value:
                raise ValueError(f"{path} is stale; regenerate reheat alarm reference")
        else:
            path.write_text(value)
    print("OBC reheat low-discharge reference is current")


if __name__ == "__main__":
    main()
