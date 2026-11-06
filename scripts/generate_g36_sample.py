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
    "Measured primary airflow and discharge temperature feed an editable terminal reheat "
    "temperature limit and two timed teaching alarms. This is a G36-inspired subset; "
    "trim-and-respond, modulating economizer, full G36 alarms, and CO2 sequences are not modeled."
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
    "seconds_until_occupied", unit="s", label="Time Until Next Occupied Schedule",
    brick="Point", description="Calendar time until the next scheduled occupied sample",
))
graph["outputs"].append(port(
    "preconditioning_mode", "Boolean", "unitless", "Preconditioning Mode Command",
    "Point", "Computed by the editable schedule lookahead CDL sequence",
))
graph["blocks"].extend([
    constant("preconditioning_horizon", 21601.0, "s", "Start conditioning within six hours of scheduled occupancy."),
    binary("within_preconditioning_horizon", "Less", note="Compare calendar time until occupancy with the configured horizon."),
    {"name": "schedule_unoccupied", "block_type": "Not", "category": "elementary",
     "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
     "notes": "Preconditioning can run only before the scheduled occupied period."},
    binary("preconditioning_enabled", "And", "Boolean", note="Editable preconditioning mode decision."),
])
graph["connections"].extend({"from": source, "to": target} for source, target in [
    ("seconds_until_occupied", "within_preconditioning_horizon.u1"),
    ("preconditioning_horizon.y", "within_preconditioning_horizon.u2"),
    ("schedule_occupied", "schedule_unoccupied.u"),
    ("within_preconditioning_horizon.y", "preconditioning_enabled.u1"),
    ("schedule_unoccupied.y", "preconditioning_enabled.u2"),
    ("preconditioning_enabled.y", "preconditioning_mode"),
])

for index in range(1, 11):
    name = f"zone_{index:02}"
    graph["inputs"].append(port(
        f"{name}_occupied", "Boolean", "unitless", f"Zone {index} Occupancy Sensor",
        "Occupancy_Sensor", "Raw synthetic zone presence, sampled every 15 minutes",
    ))
    graph["inputs"].extend([
        port(f"{name}_measured_airflow", unit="normalized airflow", label=f"Zone {index} Primary Air Flow Sensor", brick="Supply_Air_Flow_Sensor", description="Delivered primary air from the previous physical-model step"),
        port(f"{name}_discharge_temp", unit="°C", label=f"Zone {index} Discharge Air Temperature Sensor", brick="Discharge_Air_Temperature_Sensor", description="Temperature after terminal reheat from the previous physical-model step"),
    ])
    graph["outputs"].append(port(
        f"{name}_occupancy_confirmed", "Boolean", "unitless",
        f"Zone {index} Confirmed Occupancy", "Point",
        "Result of the editable CDL one-minute arrival and five-minute vacancy sequence",
    ))
    zone = next(block for block in graph["blocks"] if block["name"] == name)
    zone["description"] = "G36-style zone setpoint and minimum-airflow reset with local occupancy sensing."
    zone["notes"] = (
        "The scheduled mode uses occupied 21/24 °C setpoints. Five minutes without a "
        "person widens both setpoints by 0.5 K and reduces ventilation minimums; one "
        "minute of presence restores them. Outside the occupied schedule, the zone "
        "uses 16/32 °C setpoints and zero ventilation minimum. Before occupancy, "
        "preconditioning restores occupied temperature setpoints but keeps the reduced "
        "ventilation minimum. Heating mode can open the damper above minimum as the PI demand rises. CDL TrueDelay "
        "and Latch blocks confirm arrival and vacancy from the raw occupancy sensor. Measured "
        "airflow and discharge temperature are fed back from the physical model. A CDL "
        "temperature-derived valve cap limits reheat, and five-minute low-flow and "
        "high-discharge alarms report persistent faults."
    )
    zone["inputs"].extend([
        port("scheduled_occupied", "Boolean", "unitless", "Occupied schedule", "Point"),
        port("preconditioning_mode", "Boolean", "unitless", "Preconditioning mode", "Point"),
        port("zone_presence_raw", "Boolean", "unitless", "Raw Zone Occupancy Sensor", "Occupancy_Sensor"),
        port("measured_airflow", unit="normalized airflow", label="Primary Air Flow Sensor", brick="Supply_Air_Flow_Sensor"),
        port("discharge_temp", unit="°C", label="Discharge Air Temperature Sensor", brick="Discharge_Air_Temperature_Sensor"),
    ])
    zone["outputs"].append(port(
        "occupancy_confirmed", "Boolean", "unitless", "Confirmed Occupancy", "Point",
    ))
    zone["outputs"].extend([
        port("low_airflow_alarm", "Boolean", "unitless", "Low Primary Air Flow Alarm", "Point"),
        port("high_discharge_alarm", "Boolean", "unitless", "High Discharge Temperature Alarm", "Point"),
    ])
    graph["outputs"].extend([
        port(f"{name}_low_airflow_alarm", "Boolean", "unitless", f"Zone {index} Low Primary Air Flow Alarm", "Point"),
        port(f"{name}_high_discharge_alarm", "Boolean", "unitless", f"Zone {index} High Discharge Temperature Alarm", "Point"),
    ])
    graph["connections"].extend([
        {"from": "schedule_occupied", "to": f"{name}.scheduled_occupied"},
        {"from": "preconditioning_enabled.y", "to": f"{name}.preconditioning_mode"},
        {"from": f"{name}_occupied", "to": f"{name}.zone_presence_raw"},
        {"from": f"{name}.occupancy_confirmed", "to": f"{name}_occupancy_confirmed"},
        {"from": f"{name}_measured_airflow", "to": f"{name}.measured_airflow"},
        {"from": f"{name}_discharge_temp", "to": f"{name}.discharge_temp"},
        {"from": f"{name}.low_airflow_alarm", "to": f"{name}_low_airflow_alarm"},
        {"from": f"{name}.high_discharge_alarm", "to": f"{name}_high_discharge_alarm"},
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
        constant("maximum_discharge_temp", 35.0, "°C", "Editable terminal discharge temperature upper target; physical coil safety remains at 40 °C."),
        {"name": "discharge_headroom", "block_type": "Subtract", "category": "elementary", "inputs": [port("u1", unit="°C"), port("u2", unit="°C")], "outputs": [port("y", unit="K")], "notes": "Temperature rise allowed above measured AHU supply air."},
        {"name": "reheat_capacity_per_flow", "block_type": "MultiplyByParameter", "category": "elementary", "inputs": [port("u", unit="K")], "outputs": [port("y", unit="fraction (0–1) / normalized airflow")], "parameters": [{"name": "k", "type": "Real", "value": 0.15, "unit": "normalized airflow⁻¹ K⁻¹"}], "notes": "Converts temperature headroom to valve capacity: 1.2 kW per airflow-K divided by the 8 kW reheat coil."},
        binary("reheat_flow_cap", "Multiply", "Real", "Real", "Scale valve cap by measured primary airflow."),
        {"name": "reheat_cap_limit", "block_type": "Limiter", "category": "elementary", "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", unit="fraction (0–1)")], "parameters": [{"name": "uMin", "value": 0.0}, {"name": "uMax", "value": 1.0}], "notes": "Keep the discharge-temperature-derived valve cap between closed and open."},
        binary("reheat_capped", "Min", "Real", "Real", "Limit zone heating command with the discharge temperature cap."),
        constant("low_flow_fraction", 0.5, "fraction (0–1)", "Alarm when measured flow stays below half of its request."),
        binary("low_flow_threshold", "Multiply", "Real", "Real", "Half of requested primary airflow."),
        binary("low_flow_detected", "Less", note="Measured primary airflow is below half the request."),
        constant("flow_alarm_enable_threshold", 0.1, "normalized airflow", "Avoid alarm when the VAV is not requesting air."),
        binary("flow_requested", "Greater", note="Only monitor low flow while the box requests air."),
        binary("low_flow_condition", "And", "Boolean", note="Request exists and delivered primary flow is low."),
        {"name": "low_flow_delay", "block_type": "Buildings.Controls.OBC.CDL.Logical.TrueDelay", "category": "elementary", "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")], "parameters": [{"name": "delayTime", "value": 300.0, "unit": "s"}, {"name": "delayOnInit", "value": True, "type": "Boolean"}], "notes": "Require five continuous minutes of deficient primary flow before alarm."},
        binary("high_discharge_detected", "Greater", note="Measured discharge temperature exceeds the editable upper target."),
        {"name": "high_discharge_delay", "block_type": "Buildings.Controls.OBC.CDL.Logical.TrueDelay", "category": "elementary", "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")], "parameters": [{"name": "delayTime", "value": 300.0, "unit": "s"}, {"name": "delayOnInit", "value": True, "type": "Boolean"}], "notes": "Require five continuous minutes above the discharge upper target before alarm."},
        constant("standby_cool", 24.5, "°C", "Occupied standby cooling setpoint after five minutes vacant."),
        constant("standby_heat", 20.5, "°C", "Occupied standby heating setpoint after five minutes vacant."),
        constant("unoccupied_cool", 32.0, "°C", "G36 unoccupied cooling setback setpoint."),
        constant("unoccupied_heat", 16.0, "°C", "G36 unoccupied heating setback setpoint."),
        constant("standby_damper", 0.05, "fraction (0–1)", "Reduced occupied standby ventilation minimum."),
        constant("unoccupied_damper", 0.0, "fraction (0–1)", "No ventilation minimum outside occupied schedule."),
        constant("standby_airflow", 0.05, "normalized airflow", "Reduced occupied standby airflow minimum."),
        constant("unoccupied_airflow", 0.0, "normalized airflow", "No minimum airflow outside occupied schedule."),
        {"name": "presence_delay", "block_type": "Buildings.Controls.OBC.CDL.Logical.TrueDelay", "category": "elementary",
         "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
         "parameters": [{"name": "delayTime", "value": 60.0, "unit": "s"}, {"name": "delayOnInit", "value": True, "type": "Boolean"}],
         "notes": "Require one continuous minute of presence before setting occupied."},
        {"name": "presence_inverse", "block_type": "Buildings.Controls.OBC.CDL.Logical.Not", "category": "elementary",
         "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
         "notes": "True when the raw occupancy sensor reports vacant."},
        {"name": "vacancy_delay", "block_type": "Buildings.Controls.OBC.CDL.Logical.TrueDelay", "category": "elementary",
         "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
         "parameters": [{"name": "delayTime", "value": 300.0, "unit": "s"}, {"name": "delayOnInit", "value": True, "type": "Boolean"}],
         "notes": "Require five continuous minutes vacant before clearing occupied."},
        {"name": "occupancy_latch", "block_type": "Buildings.Controls.OBC.CDL.Logical.Latch", "category": "elementary",
         "inputs": [port("u", "Boolean"), port("clr", "Boolean")], "outputs": [port("y", "Boolean")],
         "notes": "Hold confirmed occupancy until the vacancy delay clears it."},
    ])
    blocks.append({
        "name": "conditioning_mode", "block_type": "Or", "category": "elementary",
        "inputs": [port("u1", "Boolean"), port("u2", "Boolean")],
        "outputs": [port("y", "Boolean")],
        "notes": "Request occupied temperature setpoints during the schedule or the preconditioning window.",
    })
    edges = zone["connections"]
    edges.extend([
        {"from": "maximum_discharge_temp.y", "to": "discharge_headroom.u1"},
        {"from": "supply_air_temp", "to": "discharge_headroom.u2"},
        {"from": "discharge_headroom.y", "to": "reheat_capacity_per_flow.u"},
        {"from": "reheat_capacity_per_flow.y", "to": "reheat_flow_cap.u1"},
        {"from": "measured_airflow", "to": "reheat_flow_cap.u2"},
        {"from": "reheat_flow_cap.y", "to": "reheat_cap_limit.u"},
        {"from": "reheat_select.y", "to": "reheat_capped.u1"},
        {"from": "reheat_cap_limit.y", "to": "reheat_capped.u2"},
        {"from": "airflow_request_sum.y", "to": "low_flow_threshold.u1"},
        {"from": "low_flow_fraction.y", "to": "low_flow_threshold.u2"},
        {"from": "measured_airflow", "to": "low_flow_detected.u1"},
        {"from": "low_flow_threshold.y", "to": "low_flow_detected.u2"},
        {"from": "airflow_request_sum.y", "to": "flow_requested.u1"},
        {"from": "flow_alarm_enable_threshold.y", "to": "flow_requested.u2"},
        {"from": "low_flow_detected.y", "to": "low_flow_condition.u1"},
        {"from": "flow_requested.y", "to": "low_flow_condition.u2"},
        {"from": "low_flow_condition.y", "to": "low_flow_delay.u"},
        {"from": "low_flow_delay.y", "to": "low_airflow_alarm"},
        {"from": "discharge_temp", "to": "high_discharge_detected.u1"},
        {"from": "maximum_discharge_temp.y", "to": "high_discharge_detected.u2"},
        {"from": "high_discharge_detected.y", "to": "high_discharge_delay.u"},
        {"from": "high_discharge_delay.y", "to": "high_discharge_alarm"},
        {"from": "scheduled_occupied", "to": "conditioning_mode.u1"},
        {"from": "preconditioning_mode", "to": "conditioning_mode.u2"},
        {"from": "zone_presence_raw", "to": "presence_delay.u"},
        {"from": "zone_presence_raw", "to": "presence_inverse.u"},
        {"from": "presence_inverse.y", "to": "vacancy_delay.u"},
        {"from": "presence_delay.y", "to": "occupancy_latch.u"},
        {"from": "vacancy_delay.y", "to": "occupancy_latch.clr"},
        {"from": "occupancy_latch.y", "to": "occupancy_confirmed"},
    ])
    for edge in edges:
        if edge["from"] == "reheat_select.y" and edge["to"] == "reheat_valve_position":
            edge["from"] = "reheat_capped.y"
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
            {"from": "occupancy_latch.y", "to": f"{stem}_standby.u3"},
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

    blocks.extend([
        {"name": "heating_damper_gain", "block_type": "MultiplyByParameter", "category": "elementary", "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", unit="fraction (0–1)")], "parameters": [{"name": "k", "value": 0.85, "type": "Real"}], "notes": "Increase primary-air damper opening as the heating PI demand rises."},
        {"name": "heating_damper_offset", "block_type": "Add", "category": "elementary", "inputs": [port("u1", unit="fraction (0–1)"), port("u2", unit="fraction (0–1)")], "outputs": [port("y", unit="fraction (0–1)")], "notes": "Add heating demand to the zone's active minimum damper position."},
        {"name": "heating_damper_limit", "block_type": "Limiter", "category": "elementary", "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", unit="fraction (0–1)")], "parameters": [{"name": "uMin", "value": 0.0}, {"name": "uMax", "value": 1.0}], "notes": "Keep heating damper command within its physical travel."},
        switch("heating_damper_select", "fraction (0–1)", "Use the heating airflow path during a heating call; otherwise retain the cooling/ventilation path."),
    ])
    edges.extend([
        {"from": "heat_pi.y", "to": "heating_damper_gain.u"},
        {"from": "minimum_damper_heating.y", "to": "heating_damper_offset.u1"},
        {"from": "heating_damper_gain.y", "to": "heating_damper_offset.u2"},
        {"from": "heating_damper_offset.y", "to": "heating_damper_limit.u"},
        {"from": "heating_damper_limit.y", "to": "heating_damper_select.u1"},
        {"from": "damper_select.y", "to": "heating_damper_select.u2"},
        {"from": "heating_mode.y", "to": "heating_damper_select.u3"},
    ])
    for edge in edges:
        if edge["from"] == "damper_select.y" and edge["to"] in ("damper_position", "flow_span.u"):
            edge["from"] = "heating_damper_select.y"

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
    graph["connections"].extend([
        {"from": f"{prefix}_return_air_temp", "to": f"{prefix}.return_air_temp"},
    ])
    controller["inputs"].append(port(
        "return_air_temp", unit="°C", label="Return Air Temperature Sensor",
        brick="Return_Air_Temperature_Sensor",
    ))
    controller["blocks"].extend([
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
    controller["connections"] = [
        edge for edge in controller["connections"]
        if not (edge["from"] == "minimum_outdoor_air.y" and edge["to"] == "outdoor_air_fraction")
    ]
    controller["notes"] += " An editable temperature enable sequence selects 20% or 80% outdoor air for the physical mixing box."

(EXAMPLES / "four_pipe_g36.json").write_text(json.dumps(graph, indent=2) + "\n")
