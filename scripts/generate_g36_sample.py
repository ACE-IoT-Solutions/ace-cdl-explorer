"""Build the occupancy-aware four-pipe example from the conventional graph."""

import copy
import json
from pathlib import Path

EXAMPLES = Path(__file__).resolve().parents[1] / "web/public/examples"
source = json.loads((EXAMPLES / "four_pipe_building.json").read_text())
graph = copy.deepcopy(source)
graph["name"] = "FourPipeG36Controls"
graph["description"] = "Simplified G36-style occupied, standby, and unoccupied zone control on the same four-pipe building."
graph["notes"] = (
    "Compare this sample with FourPipeBuildingControls using the same weather, equipment, "
    "and physical model. Each zone has a debounced Brick occupancy sensor. Occupied setpoints "
    "are 21/24 °C, standby widens them by 0.5 K, and unoccupied setpoints are 16/32 °C. "
    "Ventilation minimums and AHU fan minimums also reset. A six-hour preconditioning "
    "window requests occupied temperature setpoints before scheduled occupancy. The AHUs "
    "use outdoor/return temperature comparisons to enable a simplified free-cooling mode. "
    "This is a G36-inspired subset; trim-and-respond, modulating economizer, alarms, "
    "and CO2 sequences are not modeled."
)


def port(name, kind="Real", unit=None, label=None, brick=None, description=None):
    result = {"name": name, "type": kind}
    if label:
        result["label"] = label
    if unit:
        result["unit"] = unit
    if description:
        result["description"] = description
    if brick:
        result["brick_class"] = brick
    return result


def constant(name, value, unit, note):
    return {
        "name": name, "block_type": "Constant", "category": "elementary",
        "inputs": [], "outputs": [port("y", unit=unit)],
        "parameters": [{"name": "k", "type": "Real", "value": value, "unit": unit}],
        "notes": note,
    }


def switch(name, unit, note):
    return {
        "name": name, "block_type": "Switch", "category": "elementary",
        "inputs": [port("u1", unit=unit), port("u2", unit=unit), port("u3", "Boolean")],
        "outputs": [port("y", unit=unit)], "notes": note,
    }


def binary(name, kind, input_type="Real", output_type="Boolean", note=""):
    return {
        "name": name, "block_type": kind, "category": "elementary",
        "inputs": [port("u1", input_type), port("u2", input_type)],
        "outputs": [port("y", output_type)], "notes": note,
    }


graph["inputs"].append(port(
    "schedule_occupied", "Boolean", "unitless", "Occupied schedule",
    "Point", "Building occupied schedule from the weather year's 15-minute calendar",
))
graph["inputs"].append(port(
    "preconditioning_mode", "Boolean", "unitless", "Preconditioning mode",
    "Point", "True for up to six hours before the next scheduled occupied interval",
))

for index in range(1, 11):
    name = f"zone_{index:02}"
    graph["inputs"].append(port(
        f"{name}_occupied", "Boolean", "unitless", f"Zone {index} Occupancy Sensor",
        "Occupancy_Sensor", "Synthetic zone presence, debounced for five minutes vacant and one minute present",
    ))
    zone = next(block for block in graph["blocks"] if block["name"] == name)
    zone["description"] = "G36-style zone setpoint and minimum-airflow reset with local occupancy sensing."
    zone["notes"] = (
        "The scheduled mode uses occupied 21/24 °C setpoints. Five minutes without a "
        "person widens both setpoints by 0.5 K and reduces ventilation minimums; one "
        "minute of presence restores them. Outside the occupied schedule, the zone "
        "uses 16/32 °C setpoints and zero ventilation minimum. Before occupancy, "
        "preconditioning restores occupied temperature setpoints but keeps the reduced "
        "ventilation minimum. Heating mode preserves a minimum airflow path. The sensor debounce "
        "is performed by the simulation input, while these switches are CDL blocks."
    )
    zone["inputs"].extend([
        port("scheduled_occupied", "Boolean", "unitless", "Occupied schedule", "Point"),
        port("preconditioning_mode", "Boolean", "unitless", "Preconditioning mode", "Point"),
        port("zone_occupied", "Boolean", "unitless", "Zone Occupancy Sensor", "Occupancy_Sensor"),
    ])
    graph["connections"].extend([
        {"from": "schedule_occupied", "to": f"{name}.scheduled_occupied"},
        {"from": "preconditioning_mode", "to": f"{name}.preconditioning_mode"},
        {"from": f"{name}_occupied", "to": f"{name}.zone_occupied"},
    ])
    blocks = zone["blocks"]
    by_name = {block["name"]: block for block in blocks}
    by_name["cool_setpoint"]["parameters"][0]["value"] = 24.0
    by_name["cool_setpoint"]["notes"] = "G36 occupied cooling setpoint: 24 °C."
    by_name["heat_setpoint"]["notes"] = "G36 occupied heating setpoint: 21 °C."
    by_name["damper_limit"]["parameters"] = [
        {**parameter, "value": 0.0} if parameter["name"] == "uMin" else parameter
        for parameter in by_name["damper_limit"]["parameters"]
    ]
    blocks.extend([
        constant("standby_cool", 24.5, "°C", "Occupied standby cooling setpoint after five minutes vacant."),
        constant("standby_heat", 20.5, "°C", "Occupied standby heating setpoint after five minutes vacant."),
        constant("unoccupied_cool", 32.0, "°C", "G36 unoccupied cooling setback setpoint."),
        constant("unoccupied_heat", 16.0, "°C", "G36 unoccupied heating setback setpoint."),
        constant("standby_damper", 0.05, "fraction (0–1)", "Reduced occupied standby ventilation minimum."),
        constant("unoccupied_damper", 0.0, "fraction (0–1)", "No ventilation minimum outside occupied schedule."),
        constant("standby_airflow", 0.05, "normalized airflow", "Reduced occupied standby airflow minimum."),
        constant("unoccupied_airflow", 0.0, "normalized airflow", "No minimum airflow outside occupied schedule."),
    ])
    blocks.append({
        "name": "conditioning_mode", "block_type": "Or", "category": "elementary",
        "inputs": [port("u1", "Boolean"), port("u2", "Boolean")],
        "outputs": [port("y", "Boolean")],
        "notes": "Request occupied temperature setpoints during the schedule or the preconditioning window.",
    })
    edges = zone["connections"]
    edges.extend([
        {"from": "scheduled_occupied", "to": "conditioning_mode.u1"},
        {"from": "preconditioning_mode", "to": "conditioning_mode.u2"},
    ])
    for stem, unit in [
        ("cool_setpoint", "°C"), ("heat_setpoint", "°C"),
        ("minimum_damper", "fraction (0–1)"),
        ("minimum_airflow", "normalized airflow"),
    ]:
        blocks.append(switch(f"{stem}_standby", unit, "Select occupied or sensor-vacant standby value."))
        blocks.append(switch(f"{stem}_active", unit, "Select scheduled or unoccupied value."))
    rewired = {"cool_setpoint": 0, "heat_setpoint": 0, "minimum_damper": 0, "minimum_airflow": 0}
    for edge in edges:
        for stem in rewired:
            if edge.get("from") == f"{stem}.y":
                edge["from"] = f"{stem}_active.y"
                rewired[stem] += 1
    assert all(rewired.values()), (name, rewired)
    alternatives = {
        "cool_setpoint": ("standby_cool", "unoccupied_cool"),
        "heat_setpoint": ("standby_heat", "unoccupied_heat"),
        "minimum_damper": ("standby_damper", "unoccupied_damper"),
        "minimum_airflow": ("standby_airflow", "unoccupied_airflow"),
    }
    for stem, (standby, unoccupied) in alternatives.items():
        edges.extend([
            {"from": f"{stem}.y", "to": f"{stem}_standby.u1"},
            {"from": f"{standby}.y", "to": f"{stem}_standby.u2"},
            {"from": "zone_occupied", "to": f"{stem}_standby.u3"},
            {"from": f"{stem}_standby.y", "to": f"{stem}_active.u1"},
            {"from": f"{unoccupied}.y", "to": f"{stem}_active.u2"},
            {"from": "conditioning_mode.y" if stem in ("cool_setpoint", "heat_setpoint") else "scheduled_occupied", "to": f"{stem}_active.u3"},
        ])
    for stem, unit in (("minimum_damper", "fraction (0–1)"), ("minimum_airflow", "normalized airflow")):
        blocks.append(switch(f"{stem}_heating", unit, "Preserve minimum discharge airflow during a heating call."))
        edges.extend([
            {"from": f"{stem}.y", "to": f"{stem}_heating.u1"},
            {"from": f"{stem}_active.y", "to": f"{stem}_heating.u2"},
            {"from": "heating_mode.y", "to": f"{stem}_heating.u3"},
        ])
        for edge in edges:
            if edge.get("from") == f"{stem}_active.y" and edge.get("to") != f"{stem}_heating.u2":
                edge["from"] = f"{stem}_heating.y"

for ahu in (1, 2):
    controller = next(block for block in graph["blocks"] if block["name"] == f"ahu_{ahu}")
    fan_limit = next(block for block in controller["blocks"] if block["name"] == "fan_limit")
    for parameter in fan_limit["parameters"]:
        if parameter["name"] == "uMin":
            parameter["value"] = 0.0
    fan_limit["notes"] = "Zero minimum fan command permits shutdown when all zone airflow requests fall to zero."
    prefix = f"ahu_{ahu}"
    graph["inputs"].append(port(
        f"{prefix}_return_air_temp", unit="°C", label=f"AHU {ahu} Return Air Temperature Sensor",
        brick="Return_Air_Temperature_Sensor",
        description="Return temperature of the five zones served by this AHU",
    ))
    graph["outputs"].append(port(
        f"{prefix}_outdoor_air_fraction", unit="fraction (0–1)",
        label=f"AHU {ahu} Outdoor Air Damper Command", brick="Damper_Position_Command",
        description="Outdoor air share used by the simulated mixing box",
    ))
    graph["connections"].extend([
        {"from": f"{prefix}_return_air_temp", "to": f"{prefix}.return_air_temp"},
        {"from": f"{prefix}.outdoor_air_fraction", "to": f"{prefix}_outdoor_air_fraction"},
    ])
    controller["inputs"].append(port(
        "return_air_temp", unit="°C", label="Return Air Temperature Sensor",
        brick="Return_Air_Temperature_Sensor",
    ))
    controller["outputs"].append(port(
        "outdoor_air_fraction", unit="fraction (0–1)",
        label="Outdoor Air Damper Command", brick="Damper_Position_Command",
    ))
    controller["blocks"].extend([
        constant("minimum_outdoor_air", 0.2, "fraction (0–1)", "Minimum outdoor-air share for this teaching model."),
        constant("free_cooling_outdoor_air", 0.8, "fraction (0–1)", "Higher outdoor-air share when free cooling is useful."),
        constant("economizer_low_limit", 6.0, "°C", "Avoid high outdoor-air intake below 6 °C in this simplified model."),
        binary("outdoor_below_return", "Less", note="Outdoor air can cool the return air."),
        binary("return_above_supply_target", "Greater", note="The AHU calls for cooling relative to return air."),
        binary("outdoor_above_low_limit", "Greater", note="Avoid low-temperature economizer operation."),
        binary("outdoor_above_supply_target", "Greater", note="Avoid high outdoor-air intake that would require heating."),
        binary("free_cooling_useful", "And", "Boolean", note="Temperature conditions allow free cooling."),
        binary("economizer_temperature_ok", "And", "Boolean", note="Outdoor air stays above the low limit and supply target."),
        binary("economizer_enabled", "And", "Boolean", note="Enable the higher outdoor-air share when all conditions hold."),
        switch("outdoor_air_select", "fraction (0–1)", "Select the outdoor-air mixing fraction."),
    ])
    controller["connections"].extend({"from": source, "to": target} for source, target in [
        ("outdoor_c", "outdoor_below_return.u1"),
        ("return_air_temp", "outdoor_below_return.u2"),
        ("return_air_temp", "return_above_supply_target.u1"),
        ("supply_reset.y", "return_above_supply_target.u2"),
        ("outdoor_c", "outdoor_above_low_limit.u1"),
        ("economizer_low_limit.y", "outdoor_above_low_limit.u2"),
        ("outdoor_c", "outdoor_above_supply_target.u1"),
        ("supply_reset.y", "outdoor_above_supply_target.u2"),
        ("outdoor_below_return.y", "free_cooling_useful.u1"),
        ("return_above_supply_target.y", "free_cooling_useful.u2"),
        ("outdoor_above_low_limit.y", "economizer_temperature_ok.u1"),
        ("outdoor_above_supply_target.y", "economizer_temperature_ok.u2"),
        ("free_cooling_useful.y", "economizer_enabled.u1"),
        ("economizer_temperature_ok.y", "economizer_enabled.u2"),
        ("free_cooling_outdoor_air.y", "outdoor_air_select.u1"),
        ("minimum_outdoor_air.y", "outdoor_air_select.u2"),
        ("economizer_enabled.y", "outdoor_air_select.u3"),
        ("outdoor_air_select.y", "outdoor_air_fraction"),
    ])
    controller["notes"] += " An editable temperature enable sequence selects 20% or 80% outdoor air for the physical mixing box."

(EXAMPLES / "four_pipe_g36.json").write_text(json.dumps(graph, indent=2) + "\n")
