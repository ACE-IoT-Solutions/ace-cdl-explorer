"""Build the occupancy-aware four-pipe example from the conventional graph."""

import copy
import json
from pathlib import Path

EXAMPLES = Path(__file__).resolve().parents[1] / "web/public/examples"
source = json.loads((EXAMPLES / "four_pipe_building.json").read_text())
graph = copy.deepcopy(source)
graph["name"] = "FourPipeG36Controls"
graph["description"] = "G36-style zone, AHU, and hot/chilled-water reset controls on the same simplified four-pipe building."
graph["notes"] = (
    "Compare this sample with FourPipeBuildingControls using the same weather, equipment, "
    "and physical model. Each zone has a debounced Brick occupancy sensor. Occupied setpoints "
    "are 21/24 °C, standby widens them by 0.5 K, and unoccupied setpoints are 16/32 °C. "
    "Ventilation minimums and AHU fan minimums also reset. Qualified VAV cooling requests "
    "drive a sampled AHU supply-air temperature reset. A six-hour preconditioning "
    "window requests occupied temperature setpoints before scheduled occupancy. The AHUs "
    "use outdoor/return temperature comparisons and measured mixed-air feedback to modulate free cooling. "
    "Measured primary airflow and discharge temperature feed an editable terminal discharge "
    "setpoint and valve loop, plus two timed teaching alarms. Timed Stage 1 AHU freeze "
    "protection overrides outdoor air and supports the hot-water plant; Stage 2 interrupts "
    "outdoor ventilation for one hour after a deeper fault. This is a G36-inspired subset; "
    "full G36 economizer limits, plant pressure reset and staging, alarms, and CO2 sequences are not modeled."
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
        "ventilation minimum. Heating mode raises airflow above minimum only after the discharge is warm, while preconditioning can use the heating maximum. CDL TrueDelay "
        "and Latch blocks confirm arrival and vacancy from the raw occupancy sensor. Measured "
        "airflow and discharge temperature are fed back from the physical model. A CDL "
        "discharge-temperature reset and valve PI loop command reheat, and five-minute low-flow and "
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
        port("pressure_reset_request", "Integer", "count", "Duct Pressure Reset Request", "Point"),
    ])
    graph["outputs"].extend([
        port(f"{name}_low_airflow_alarm", "Boolean", "unitless", f"Zone {index} Low Primary Air Flow Alarm", "Point"),
        port(f"{name}_high_discharge_alarm", "Boolean", "unitless", f"Zone {index} High Discharge Temperature Alarm", "Point"),
        port(f"{name}_pressure_reset_request", "Integer", "count", f"Zone {index} Duct Pressure Reset Request", "Point"),
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
        {"from": f"{name}.pressure_reset_request", "to": f"{name}_pressure_reset_request"},
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

    blocks.extend([
        constant("pressure_request_damper_threshold", 0.9, "fraction (0–1)", "A nearly open VAV may request more duct pressure."),
        binary("damper_nearly_open", "Greater", note="The commanded damper exceeds 90% open."),
        {"name": "pressure_request_flow_fraction", "block_type": "MultiplyByParameter", "category": "elementary", "inputs": [port("u", unit="normalized airflow")], "outputs": [port("y", unit="normalized airflow")], "parameters": [{"name": "k", "value": 0.9, "type": "Real"}], "notes": "Require measured flow to reach at least 90% of request."},
        binary("pressure_request_low_flow", "Less", note="Measured primary flow is below 90% of request."),
        binary("pressure_request_condition", "And", "Boolean", note="A nearly open damper has an airflow deficit."),
        {"name": "pressure_request_delay", "block_type": "Buildings.Controls.OBC.CDL.Logical.TrueDelay", "category": "elementary", "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")], "parameters": [{"name": "delayTime", "value": 120.0, "unit": "s"}, {"name": "delayOnInit", "value": True, "type": "Boolean"}], "notes": "Require a persistent two-minute pressure request."},
        {"name": "pressure_request_integer", "block_type": "Buildings.Controls.OBC.CDL.Conversions.BooleanToInteger", "category": "elementary", "inputs": [port("u", "Boolean")], "outputs": [port("y", "Integer", "count")], "notes": "Convert the qualified request to integer 0 or 1 for AHU aggregation."},
    ])
    edges.extend({"from": source, "to": target} for source, target in [
        ("heating_damper_select.y", "damper_nearly_open.u1"),
        ("pressure_request_damper_threshold.y", "damper_nearly_open.u2"),
        ("airflow_request_sum.y", "pressure_request_flow_fraction.u"),
        ("measured_airflow", "pressure_request_low_flow.u1"),
        ("pressure_request_flow_fraction.y", "pressure_request_low_flow.u2"),
        ("damper_nearly_open.y", "pressure_request_condition.u1"),
        ("pressure_request_low_flow.y", "pressure_request_condition.u2"),
        ("pressure_request_condition.y", "pressure_request_delay.u"),
        ("pressure_request_delay.y", "pressure_request_integer.u"),
        ("pressure_request_integer.y", "pressure_reset_request"),
    ])

for ahu in (1, 2):
    controller = next(block for block in graph["blocks"] if block["name"] == f"ahu_{ahu}")
    fan_limit = next(block for block in controller["blocks"] if block["name"] == "fan_limit")
    for parameter in fan_limit["parameters"]:
        if parameter["name"] == "uMin":
            parameter["value"] = 0.0
    fan_limit["notes"] = "Zero minimum fan command permits shutdown when all zone airflow requests fall to zero."
    next(block for block in controller["blocks"] if block["name"] == "minimum_outdoor_air")["notes"] = "Fallback outdoor-air fraction when economizing is disabled; the measured-flow minimum loop may raise it."
    prefix = f"ahu_{ahu}"
    graph["inputs"].append(port(
        f"{prefix}_return_air_temp", unit="°C", label=f"AHU {ahu} Return Air Temperature Sensor",
        brick="Return_Air_Temperature_Sensor",
        description="Return temperature of the five zones served by this AHU",
    ))
    graph["inputs"].append(port(
        f"{prefix}_duct_pressure", unit="normalized pressure", label=f"AHU {ahu} Duct Static Pressure Sensor",
        brick="Point", description="Normalized duct pressure from the previous physical-model step",
    ))
    graph["inputs"].append(port(
        f"{prefix}_outdoor_airflow", unit="normalized airflow", label=f"AHU {ahu} Outside Air Flow Sensor",
        brick="Outside_Air_Flow_Sensor", description="Delivered outside air from the previous physical-model step",
    ))
    graph["outputs"].extend([
        port(f"{prefix}_pressure_request_count", "Integer", "count", f"AHU {ahu} Duct Pressure Reset Request Count", "Point"),
        port(f"{prefix}_duct_pressure_setpoint", unit="normalized pressure", label=f"AHU {ahu} Duct Static Pressure Setpoint", brick="Point"),
        port(f"{prefix}_minimum_outdoor_airflow", unit="normalized airflow", label=f"AHU {ahu} Minimum Outside Air Flow Setpoint", brick="Outside_Air_Flow_Setpoint"),
    ])
    graph["connections"].extend([
        {"from": f"{prefix}_return_air_temp", "to": f"{prefix}.return_air_temp"},
        {"from": f"{prefix}_duct_pressure", "to": f"{prefix}.duct_pressure"},
        {"from": f"{prefix}_outdoor_airflow", "to": f"{prefix}.outdoor_airflow"},
        {"from": f"{prefix}.pressure_request_count", "to": f"{prefix}_pressure_request_count"},
        {"from": f"{prefix}.duct_pressure_setpoint", "to": f"{prefix}_duct_pressure_setpoint"},
        {"from": f"{prefix}.minimum_outdoor_airflow", "to": f"{prefix}_minimum_outdoor_airflow"},
    ])
    controller["inputs"].append(port(
        "return_air_temp", unit="°C", label="Return Air Temperature Sensor",
        brick="Return_Air_Temperature_Sensor",
    ))
    controller["inputs"].append(port("duct_pressure", unit="normalized pressure", label="Duct Static Pressure Sensor", brick="Point"))
    controller["inputs"].append(port("outdoor_airflow", unit="normalized airflow", label="Outside Air Flow Sensor", brick="Outside_Air_Flow_Sensor"))
    controller["outputs"].extend([
        port("pressure_request_count", "Integer", "count", "Zone Duct Pressure Reset Request Count", "Point"),
        port("duct_pressure_setpoint", unit="normalized pressure", label="Duct Static Pressure Setpoint", brick="Point"),
        port("minimum_outdoor_airflow", unit="normalized airflow", label="Minimum Outside Air Flow Setpoint", brick="Outside_Air_Flow_Setpoint"),
    ])
    zones = [f"zone_{index:02d}" for index in range((ahu - 1) * 5 + 1, ahu * 5 + 1)]
    for name in zones:
        controller["inputs"].append(port(f"{name}_pressure_request", "Integer", "count", f"{name} Duct Pressure Request", "Point"))
        graph["connections"].append({"from": f"{name}.pressure_reset_request", "to": f"{prefix}.{name}_pressure_request"})
    for index in range(4):
        controller["blocks"].append({
            "name": f"request_sum_{index + 1}", "block_type": "Buildings.Controls.OBC.CDL.Integers.Add", "category": "elementary",
            "inputs": [port("u1", "Integer", "count"), port("u2", "Integer", "count")],
            "outputs": [port("y", "Integer", "count")],
            "notes": "Adds qualified integer VAV pressure requests for this AHU.",
        })
    controller["blocks"].extend([
        {"name": "ignored_request_count", "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant", "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "count")], "parameters": [{"name": "k", "type": "Integer", "value": 2}], "notes": "Ignore the first two zone requests before increasing the pressure target."},
        {"name": "request_count_real", "block_type": "Buildings.Controls.OBC.CDL.Conversions.IntegerToReal", "category": "elementary", "inputs": [port("u", "Integer", "count")], "outputs": [port("y", unit="count")], "notes": "Convert integer zone request count for the continuous reset calculation."},
        {"name": "ignored_request_real", "block_type": "Buildings.Controls.OBC.CDL.Conversions.IntegerToReal", "category": "elementary", "inputs": [port("u", "Integer", "count")], "outputs": [port("y", unit="count")], "notes": "Convert ignored integer requests for subtraction."},
        binary("excess_request_count", "Subtract", "Real", "Real", "Subtract ignored requests from the measured zone request count."),
        constant("zero_requests", 0.0, "count", "Only requests above the ignored count trigger a response."),
        binary("positive_request_count", "Max", "Real", "Real", "Clamp excess requests to zero."),
        {"name": "response_per_request", "block_type": "MultiplyByParameter", "category": "elementary", "inputs": [port("u", unit="count")], "outputs": [port("y", unit="normalized pressure")], "parameters": [{"name": "k", "value": 0.05, "type": "Real"}], "notes": "Raise normalized pressure by 0.05 per request above the ignored count each sample."},
        constant("maximum_response", 0.1, "normalized pressure", "Cap each sample's total pressure response."),
        binary("bounded_response", "Min", "Real", "Real", "Limit the total response in one sample."),
        constant("trim_amount", -0.03, "normalized pressure", "Trim pressure down when requests do not justify a response."),
        binary("net_pressure_reset", "Add", "Real", "Real", "Combine the trim with the bounded response."),
        {"name": "request_delay", "block_type": "Buildings.Controls.OBC.CDL.Logical.TrueDelay", "category": "elementary", "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")], "parameters": [{"name": "delayTime", "value": 600.0, "unit": "s"}, {"name": "delayOnInit", "value": True, "type": "Boolean"}], "notes": "Hold the initial pressure target for ten minutes after the fan is requested."},
        constant("initial_pressure_target", 0.35, "normalized pressure", "Initial pressure setpoint when fan starts or is off."),
        constant("low_pressure_target", 0.25, "normalized pressure", "Minimum normalized duct pressure target."),
        constant("high_pressure_target", 0.8, "normalized pressure", "Maximum normalized duct pressure target."),
        {"name": "pressure_previous", "block_type": "Buildings.Controls.OBC.CDL.Discrete.UnitDelay", "category": "elementary", "inputs": [port("u", unit="normalized pressure")], "outputs": [port("y", unit="normalized pressure")], "parameters": [{"name": "samplePeriod", "value": 120.0, "unit": "s"}, {"name": "y_start", "value": 0.35}], "notes": "Hold the previous sampled pressure target; this state breaks the trim-and-respond feedback loop."},
        binary("pressure_target_sum", "Add", "Real", "Real", "Apply this sample's net reset to the previous pressure target."),
        binary("pressure_above_minimum", "Max", "Real", "Real", "Limit pressure target to the editable minimum."),
        binary("pressure_below_maximum", "Min", "Real", "Real", "Limit pressure target to the editable maximum."),
        switch("pressure_candidate", "normalized pressure", "Hold the initial target until fan run time exceeds the reset delay."),
        switch("pressure_target_output", "normalized pressure", "Expose the sampled target only after the reset delay."),
        {"name": "duct_pressure_pi", "block_type": "LimPID", "category": "elementary", "inputs": [port("u_s", unit="normalized pressure"), port("u_m", unit="normalized pressure")], "outputs": [port("y", unit="fraction (0–1)")], "parameters": [{"name": "k", "value": 0.5}, {"name": "Ti", "value": 1800.0, "unit": "s"}, {"name": "yMin", "value": 0.0}, {"name": "yMax", "value": 1.0}, {"name": "reverseActing", "type": "Boolean", "value": True}], "notes": "Modulate fan speed to measured normalized duct pressure."},
        binary("fan_feedforward_max", "Max", "Real", "Real", "Use the larger of airflow feedforward and duct-pressure PI fan commands."),
        constant("fan_request_threshold", 0.05, "normalized airflow", "Keep the fan off when the AHU has no airflow request."),
        binary("fan_requested", "Greater", note="Enable the fan when requested airflow exceeds the off threshold."),
        constant("fan_off", 0.0, "fraction (0–1)", "Zero fan speed when no VAV requests air."),
        switch("fan_enable", "fraction (0–1)", "Gate the fan command with actual VAV airflow demand."),
        {"name": "minimum_oa_flow_gain", "block_type": "MultiplyByParameter", "category": "elementary", "inputs": [port("u", unit="normalized airflow")], "outputs": [port("y", unit="normalized airflow")], "parameters": [{"name": "k", "type": "Real", "value": 0.2}], "notes": "Set a teaching minimum of 20% of AHU requested airflow as outside air; not yet ASHRAE 62.1 calculation."},
        {"name": "minimum_oa_pi", "block_type": "LimPID", "category": "elementary", "inputs": [port("u_s", unit="normalized airflow"), port("u_m", unit="normalized airflow")], "outputs": [port("y", unit="fraction (0–1)")], "parameters": [{"name": "k", "value": 0.2}, {"name": "Ti", "value": 3600.0, "unit": "s"}, {"name": "yMin", "value": 0.0}, {"name": "yMax", "value": 1.0}, {"name": "reverseActing", "type": "Boolean", "value": True}], "notes": "Increase outside-air damper command when measured flow misses its minimum setpoint."},
        binary("minimum_oa_command", "Max", "Real", "Real", "Apply the greater of economizer and minimum outside-air flow commands."),
    ])
    for index, name in enumerate(zones):
        source = f"{name}_pressure_request"
        target = "request_sum_1.u1" if index == 0 else f"request_sum_{index}.u2"
        controller["connections"].append({"from": source, "to": target})
        if index >= 2:
            controller["connections"].append({"from": f"request_sum_{index - 1}.y", "to": f"request_sum_{index}.u1"})
    controller["connections"].extend({"from": source, "to": target} for source, target in [
        ("request_sum_4.y", "pressure_request_count"),
        ("request_sum_4.y", "request_count_real.u"),
        ("ignored_request_count.y", "ignored_request_real.u"),
        ("request_count_real.y", "excess_request_count.u1"),
        ("ignored_request_real.y", "excess_request_count.u2"),
        ("excess_request_count.y", "positive_request_count.u1"),
        ("zero_requests.y", "positive_request_count.u2"),
        ("positive_request_count.y", "response_per_request.u"),
        ("response_per_request.y", "bounded_response.u1"),
        ("maximum_response.y", "bounded_response.u2"),
        ("bounded_response.y", "net_pressure_reset.u1"),
        ("trim_amount.y", "net_pressure_reset.u2"),
        ("pressure_previous.y", "pressure_target_sum.u1"),
        ("net_pressure_reset.y", "pressure_target_sum.u2"),
        ("pressure_target_sum.y", "pressure_above_minimum.u1"),
        ("low_pressure_target.y", "pressure_above_minimum.u2"),
        ("pressure_above_minimum.y", "pressure_below_maximum.u1"),
        ("high_pressure_target.y", "pressure_below_maximum.u2"),
        ("pressure_below_maximum.y", "pressure_candidate.u1"),
        ("initial_pressure_target.y", "pressure_candidate.u2"),
        ("request_delay.y", "pressure_candidate.u3"),
        ("pressure_candidate.y", "pressure_previous.u"),
        ("pressure_previous.y", "pressure_target_output.u1"),
        ("initial_pressure_target.y", "pressure_target_output.u2"),
        ("request_delay.y", "pressure_target_output.u3"),
        ("pressure_target_output.y", "duct_pressure_setpoint"),
        ("pressure_target_output.y", "duct_pressure_pi.u_s"),
        ("duct_pressure", "duct_pressure_pi.u_m"),
        ("fan_limit.y", "fan_feedforward_max.u1"),
        ("duct_pressure_pi.y", "fan_feedforward_max.u2"),
        ("flow_request", "fan_requested.u1"),
        ("fan_request_threshold.y", "fan_requested.u2"),
        ("fan_feedforward_max.y", "fan_enable.u1"),
        ("fan_off.y", "fan_enable.u2"),
        ("fan_requested.y", "fan_enable.u3"),
        ("fan_requested.y", "request_delay.u"),
        ("flow_request", "minimum_oa_flow_gain.u"),
        ("minimum_oa_flow_gain.y", "minimum_outdoor_airflow"),
        ("minimum_oa_flow_gain.y", "minimum_oa_pi.u_s"),
        ("outdoor_airflow", "minimum_oa_pi.u_m"),
        ("outdoor_air_select.y", "minimum_oa_command.u1"),
        ("minimum_oa_pi.y", "minimum_oa_command.u2"),
    ])
    for edge in controller["connections"]:
        if edge["from"] == "fan_limit.y" and edge["to"] == "fan_speed":
            edge["from"] = "fan_enable.y"
    fan_gain = next(block for block in controller["blocks"] if block["name"] == "fan_gain")
    fan_gain["parameters"][0]["value"] = 0.22
    fan_gain["notes"] = "Airflow feedforward allows startup while the duct-pressure PI loop takes control."
    controller["blocks"].extend([
        {"name": "economizer_mixed_air_pi", "block_type": "LimPID", "category": "elementary", "inputs": [port("u_s", unit="°C"), port("u_m", unit="°C")], "outputs": [port("y", unit="fraction (0–1)")], "parameters": [{"name": "k", "type": "Real", "value": 0.1}, {"name": "Ti", "type": "Real", "value": 600.0, "unit": "s"}, {"name": "yMin", "type": "Real", "value": 0.0}, {"name": "yMax", "type": "Real", "value": 1.0}, {"name": "reverseActing", "type": "Boolean", "value": False}], "notes": "Modulate outside-air fraction from measured mixed-air temperature and the supply-air target while economizing."},
        constant("economizer_low_limit", 12.0, "°C", "Conservative low-temperature guard for this simplified mixed-air model; not a G36 freeze-protection sequence."),
        binary("outdoor_below_return", "Less", note="Outdoor air can cool the return air."),
        binary("return_above_supply_target", "Greater", note="The AHU calls for cooling relative to return air."),
        binary("outdoor_above_low_limit", "Greater", note="Avoid low-temperature economizer operation."),
        binary("free_cooling_useful", "And", "Boolean", note="Temperature conditions allow free cooling."),
        binary("economizer_temperature_ok", "And", "Boolean", note="Outdoor air stays above the low limit while the fan is requested."),
        binary("economizer_enabled", "And", "Boolean", note="Enable mixed-air temperature modulation when all conditions hold."),
        switch("outdoor_air_select", "fraction (0–1)", "Select the modulating economizer fraction while enabled."),
    ])
    controller["connections"].extend({"from": source, "to": target} for source, target in [
        ("outdoor_c", "outdoor_below_return.u1"),
        ("return_air_temp", "outdoor_below_return.u2"),
        ("return_air_temp", "return_above_supply_target.u1"),
        ("supply_reset.y", "return_above_supply_target.u2"),
        ("outdoor_c", "outdoor_above_low_limit.u1"),
        ("economizer_low_limit.y", "outdoor_above_low_limit.u2"),
        ("supply_reset.y", "economizer_mixed_air_pi.u_s"),
        ("mixed_air_temp", "economizer_mixed_air_pi.u_m"),
        ("outdoor_below_return.y", "free_cooling_useful.u1"),
        ("return_above_supply_target.y", "free_cooling_useful.u2"),
        ("outdoor_above_low_limit.y", "economizer_temperature_ok.u1"),
        ("fan_requested.y", "economizer_temperature_ok.u2"),
        ("free_cooling_useful.y", "economizer_enabled.u1"),
        ("economizer_temperature_ok.y", "economizer_enabled.u2"),
        ("economizer_mixed_air_pi.y", "outdoor_air_select.u1"),
        ("minimum_outdoor_air.y", "outdoor_air_select.u2"),
        ("economizer_enabled.y", "outdoor_air_select.u3"),
        ("outdoor_air_select.y", "outdoor_air_fraction"),
    ])
    controller["connections"] = [
        edge for edge in controller["connections"]
        if not (edge["from"] == "minimum_outdoor_air.y" and edge["to"] == "outdoor_air_fraction")
    ]
    for edge in controller["connections"]:
        if edge["from"] == "outdoor_air_select.y" and edge["to"] == "outdoor_air_fraction":
            edge["from"] = "minimum_oa_command.y"
    controller["notes"] += " An editable mixed-air PI loop modulates outdoor air when temperature conditions permit free cooling; minimum outside-air flow has priority."

def add_chilled_water_reset():
    """Connect AHU coil requests to a sampled, editable plant reset sequence."""
    plant = next(block for block in graph["blocks"] if block["name"] == "plant")
    blocks, edges = plant["blocks"], plant["connections"]
    def wire(source, target):
        edges.append({"from": source, "to": target})
    def int_constant(name, value, note):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
                "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "count")],
                "parameters": [{"name": "k", "type": "Integer", "value": value}], "notes": note}
    def int_switch(name, note):
        return {"name": name, "block_type": "Switch", "category": "elementary",
                "inputs": [port("u1", "Integer", "count"), port("u2", "Integer", "count"), port("u3", "Boolean")],
                "outputs": [port("y", "Integer", "count")], "notes": note}
    for ahu in (1, 2):
        prefix = f"ahu_{ahu}"
        for point, source in (("cooling_valve", f"{prefix}.cooling_coil_valve_position"),
                              ("supply_setpoint_c", f"{prefix}.supply_air_temp"),
                              ("supply_actual_c", f"{prefix}_actual_supply_air_temp")):
            name = f"{prefix}_{point}"
            unit = "fraction (0–1)" if point == "cooling_valve" else "°C"
            brick = {"cooling_valve": "Cooling_Valve_Command", "supply_setpoint_c": "Supply_Air_Temperature_Setpoint",
                     "supply_actual_c": "Supply_Air_Temperature_Sensor"}[point]
            plant["inputs"].append(port(name, unit=unit, label=f"AHU {ahu} {point.replace('_', ' ').title()}", brick=brick))
            graph["connections"].append({"from": source, "to": f"plant.{name}"})
        blocks.extend([
            {"name": f"{prefix}_valve_request", "block_type": "Hysteresis", "category": "elementary",
             "inputs": [port("u", unit="fraction (0–1)")],
             "outputs": [port("y", "Boolean")], "parameters": [{"name": "uLow", "value": 0.85}, {"name": "uHigh", "value": 0.95}],
             "notes": "One chilled-water reset request when the cooling valve exceeds 95%; clear below 85%."},
            {"name": f"{prefix}_valve_request_count", "block_type": "BooleanToInteger", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Integer", "count")],
             "notes": "Convert the qualified valve request to integer zero or one."},
            binary(f"{prefix}_supply_error", "Subtract", "Real", "Real", "Actual supply air temperature minus its target."),
            constant(f"{prefix}_two_request_threshold", 2.0, "K", "Two requests after a persistent 2 K cooling deficit."),
            constant(f"{prefix}_three_request_threshold", 3.0, "K", "Three requests after a persistent 3 K cooling deficit."),
            binary(f"{prefix}_two_request_error", "Greater", note="Supply air misses its target by more than 2 K."),
            binary(f"{prefix}_three_request_error", "Greater", note="Supply air misses its target by more than 3 K."),
            *({"name": f"{prefix}_{level}_request_delay", "block_type": "TrueDelay", "category": "elementary",
               "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
               "parameters": [{"name": "delayTime", "value": 120.0, "unit": "s"}, {"name": "delayOnInit", "type": "Boolean", "value": True}],
               "notes": f"Require two minutes of {level}-request supply-air error."} for level in ("two", "three")),
            int_constant(f"{prefix}_request_two", 2, "Two chilled-water reset requests."),
            int_constant(f"{prefix}_request_three", 3, "Three chilled-water reset requests."),
            int_switch(f"{prefix}_select_two_requests", "Raise one valve request to two after a persistent supply-air error."),
            int_switch(f"{prefix}_select_three_requests", "Raise the AHU request to three after a larger persistent error."),
        ])
        for source, target in [
            (f"{prefix}_cooling_valve", f"{prefix}_valve_request.u"),
            (f"{prefix}_valve_request.y", f"{prefix}_valve_request_count.u"),
            (f"{prefix}_supply_actual_c", f"{prefix}_supply_error.u1"),
            (f"{prefix}_supply_setpoint_c", f"{prefix}_supply_error.u2"),
            (f"{prefix}_supply_error.y", f"{prefix}_two_request_error.u1"),
            (f"{prefix}_supply_error.y", f"{prefix}_three_request_error.u1"),
            (f"{prefix}_two_request_threshold.y", f"{prefix}_two_request_error.u2"),
            (f"{prefix}_three_request_threshold.y", f"{prefix}_three_request_error.u2"),
            (f"{prefix}_two_request_error.y", f"{prefix}_two_request_delay.u"),
            (f"{prefix}_three_request_error.y", f"{prefix}_three_request_delay.u"),
            (f"{prefix}_request_two.y", f"{prefix}_select_two_requests.u1"),
            (f"{prefix}_valve_request_count.y", f"{prefix}_select_two_requests.u2"),
            (f"{prefix}_two_request_delay.y", f"{prefix}_select_two_requests.u3"),
            (f"{prefix}_request_three.y", f"{prefix}_select_three_requests.u1"),
            (f"{prefix}_select_two_requests.y", f"{prefix}_select_three_requests.u2"),
            (f"{prefix}_three_request_delay.y", f"{prefix}_select_three_requests.u3"),
        ]:
            wire(source, target)
    blocks.extend([
        {"name": "chilled_reset_request_count", "block_type": "Buildings.Controls.OBC.CDL.Integers.Add", "category": "elementary",
         "inputs": [port("u1", "Integer", "count"), port("u2", "Integer", "count")],
         "outputs": [port("y", "Integer", "count")], "notes": "Sum chilled-water temperature reset requests from both AHUs."},
        {"name": "chilled_reset_request_real", "block_type": "IntegerToReal", "category": "elementary",
         "inputs": [port("u", "Integer", "count")], "outputs": [port("y", unit="count")], "notes": "Convert integer requests for sampled arithmetic."},
        constant("chilled_reset_ignored_requests", 2.0, "count", "Ignore the first two AHU requests."),
        binary("chilled_reset_excess_requests", "Subtract", "Real", "Real", "Requests above the ignored count."),
        constant("chilled_reset_zero_requests", 0.0, "count", "Clamp negative excess request count to zero."),
        binary("chilled_reset_positive_requests", "Max", "Real", "Real", "Keep only requests above the ignored count."),
        {"name": "chilled_reset_response_gain", "block_type": "MultiplyByParameter", "category": "elementary",
         "inputs": [port("u", unit="count")], "outputs": [port("y", unit="fraction (0–1)")],
         "parameters": [{"name": "k", "type": "Real", "value": 0.03}],
         "notes": "Raise normalized plant reset by 0.03 per excess request each sample."},
        constant("chilled_reset_max_response", 0.07, "fraction (0–1)", "Cap the response per sample."),
        binary("chilled_reset_bounded_response", "Min", "Real", "Real", "Cap the total response per sample."),
        constant("chilled_reset_trim", -0.02, "fraction (0–1)", "Trim reset down when requests do not justify a response."),
        binary("chilled_reset_delta", "Add", "Real", "Real", "Combine trim and response."),
        {"name": "chilled_reset_previous", "block_type": "Buildings.Controls.OBC.CDL.Discrete.UnitDelay", "category": "elementary",
         "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", unit="fraction (0–1)")],
         "parameters": [{"name": "samplePeriod", "value": 300.0, "unit": "s"}, {"name": "y_start", "value": 1.0}],
         "notes": "Hold the previous five-minute plant reset; breaks the feedback cycle."},
        binary("chilled_reset_sum", "Add", "Real", "Real", "Apply trim and response to the previous sampled reset."),
        {"name": "chilled_reset_limit", "block_type": "Limiter", "category": "elementary",
         "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", unit="fraction (0–1)")],
         "parameters": [{"name": "uMin", "value": 0.0}, {"name": "uMax", "value": 1.0}],
         "notes": "Keep the plant reset between zero and one."},
        constant("chilled_reset_initial", 1.0, "fraction (0–1)", "Restore the initial reset while chilled water is disabled."),
        switch("chilled_reset_enabled", "fraction (0–1)", "Hold the initial reset when the chilled-water loop is disabled."),
        binary("chilled_reset_inverse", "Subtract", "Real", "Real", "Available warm-water reset above the minimum supply temperature."),
        {"name": "chilled_reset_warm_delta", "block_type": "MultiplyByParameter", "category": "elementary",
         "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", unit="K")],
         "parameters": [{"name": "k", "type": "Real", "value": 5.0, "unit": "K"}],
         "notes": "Five-kelvin adjustable chilled-water supply reset range for this equipment model."},
        binary("chilled_reset_supply_sum", "Add", "Real", "Real", "Add the reset allowance to the existing editable 7°C minimum target."),
    ])
    for source, target in [
        ("ahu_1_select_three_requests.y", "chilled_reset_request_count.u1"),
        ("ahu_2_select_three_requests.y", "chilled_reset_request_count.u2"),
        ("chilled_reset_request_count.y", "chilled_reset_request_real.u"),
        ("chilled_reset_request_real.y", "chilled_reset_excess_requests.u1"),
        ("chilled_reset_ignored_requests.y", "chilled_reset_excess_requests.u2"),
        ("chilled_reset_excess_requests.y", "chilled_reset_positive_requests.u1"),
        ("chilled_reset_zero_requests.y", "chilled_reset_positive_requests.u2"),
        ("chilled_reset_positive_requests.y", "chilled_reset_response_gain.u"),
        ("chilled_reset_response_gain.y", "chilled_reset_bounded_response.u1"),
        ("chilled_reset_max_response.y", "chilled_reset_bounded_response.u2"),
        ("chilled_reset_bounded_response.y", "chilled_reset_delta.u1"),
        ("chilled_reset_trim.y", "chilled_reset_delta.u2"),
        ("chilled_reset_previous.y", "chilled_reset_sum.u1"),
        ("chilled_reset_delta.y", "chilled_reset_sum.u2"),
        ("chilled_reset_sum.y", "chilled_reset_limit.u"),
        ("chilled_reset_limit.y", "chilled_reset_enabled.u1"),
        ("chilled_reset_initial.y", "chilled_reset_enabled.u2"),
        ("chilled_water_pump_needed.y", "chilled_reset_enabled.u3"),
        ("chilled_reset_enabled.y", "chilled_reset_previous.u"),
        ("chilled_reset_initial.y", "chilled_reset_inverse.u1"),
        ("chilled_reset_previous.y", "chilled_reset_inverse.u2"),
        ("chilled_reset_inverse.y", "chilled_reset_warm_delta.u"),
        ("chilled_water_supply_setpoint.y", "chilled_reset_supply_sum.u1"),
        ("chilled_reset_warm_delta.y", "chilled_reset_supply_sum.u2"),
        ("chilled_reset_supply_sum.y", "chilled_water_supply_setpoint_c"),
        ("chilled_reset_previous.y", "chilled_water_reset_fraction"),
    ]:
        wire(source, target)
    edges[:] = [edge for edge in edges if not (edge["from"] == "chilled_water_supply_setpoint.y" and edge["to"] == "chilled_water_supply_setpoint_c")]
    edges[:] = [edge for edge in edges if not (edge["from"] == "chilled_water_supply_setpoint.y" and edge["to"] == "chilled_water_source_error.u2")]
    wire("chilled_reset_supply_sum.y", "chilled_water_source_error.u2")
    plant["outputs"].append(port("chilled_water_reset_fraction", unit="fraction (0–1)", label="Chilled Water Plant Reset", brick="Point"))
    graph["outputs"].append(port("plant_chilled_water_reset_fraction", unit="fraction (0–1)", label="Plant Chilled Water Reset", brick="Point"))
    graph["connections"].append({"from": "plant.chilled_water_reset_fraction", "to": "plant_chilled_water_reset_fraction"})
    plant["notes"] += " The G36 sample aggregates AHU chilled-water requests and resets the supply target every five minutes."


add_chilled_water_reset()


def add_hot_water_reset():
    """Add the hot-water counterpart using editable CDL request and reset blocks."""
    plant = next(block for block in graph["blocks"] if block["name"] == "plant")
    blocks, edges = plant["blocks"], plant["connections"]
    cold_blocks = copy.deepcopy(blocks)
    cold_edges = copy.deepcopy(edges)

    for ahu in (1, 2):
        prefix = f"ahu_{ahu}"
        name = f"{prefix}_heating_valve"
        plant["inputs"].append(port(name, unit="fraction (0–1)",
                                    label=f"AHU {ahu} Heating Valve Command", brick="Heating_Valve_Command"))
        graph["connections"].append({"from": f"{prefix}.heating_coil_valve_position", "to": f"plant.{name}"})
        cold_prefix = f"{prefix}_"
        request_names = {block["name"] for block in cold_blocks
                         if block["name"].startswith(cold_prefix) and
                         block["name"] not in {f"{prefix}_cooling_valve"}}
        for template in cold_blocks:
            if template["name"] not in request_names:
                continue
            cloned = copy.deepcopy(template)
            cloned["name"] = template["name"].replace(cold_prefix, f"{prefix}_hot_", 1)
            cloned["notes"] = template.get("notes", "").replace("chilled-water", "hot-water").replace("cooling", "heating")
            if cloned["name"].endswith("two_request_threshold"):
                cloned["parameters"][0]["value"] = 8.0
            elif cloned["name"].endswith("three_request_threshold"):
                cloned["parameters"][0]["value"] = 17.0
            elif cloned["name"].endswith("request_delay"):
                cloned["parameters"][0]["value"] = 300.0
            blocks.append(cloned)

        def hot_endpoint(endpoint):
            if endpoint == f"{prefix}_cooling_valve":
                return f"{prefix}_heating_valve"
            for name in request_names:
                if endpoint.startswith(name + "."):
                    return endpoint.replace(cold_prefix, f"{prefix}_hot_", 1)
            return endpoint

        for edge in cold_edges:
            target = edge["to"].split(".")[0]
            if target not in request_names:
                continue
            source, destination = hot_endpoint(edge["from"]), hot_endpoint(edge["to"])
            if destination == f"{prefix}_hot_supply_error.u1":
                source = f"{prefix}_supply_setpoint_c"
            elif destination == f"{prefix}_hot_supply_error.u2":
                source = f"{prefix}_supply_actual_c"
            edges.append({"from": source, "to": destination})

    for template in cold_blocks:
        if not template["name"].startswith("chilled_reset_"):
            continue
        cloned = copy.deepcopy(template)
        cloned["name"] = template["name"].replace("chilled_reset_", "hot_reset_", 1)
        cloned["notes"] = template.get("notes", "").replace("chilled-water", "hot-water").replace("warm-water", "hot-water")
        if cloned["name"] == "hot_reset_previous":
            next(param for param in cloned["parameters"] if param["name"] == "y_start")["value"] = 1.0 / 6.0
            cloned["notes"] = "Hold the previous five-minute hot-water reset, starting at the 45 °C nominal target."
        elif cloned["name"] == "hot_reset_initial":
            cloned["parameters"][0]["value"] = 1.0 / 6.0
            cloned["notes"] = "Restore the 45 °C nominal target while the hot-water loop is disabled."
        elif cloned["name"] == "hot_reset_warm_delta":
            cloned["parameters"][0]["value"] = -6.0
            cloned["notes"] = "Six-kelvin hot-water reset span from 44 to 50 °C around the editable 45 °C nominal target."
        elif cloned["name"] == "hot_reset_inverse":
            cloned["notes"] = "Difference between the nominal reset fraction and the sampled fraction."
        elif cloned["name"] == "hot_reset_supply_sum":
            cloned["notes"] = "Add the reset temperature offset to the editable 45 °C nominal target."
        elif cloned["name"] == "hot_reset_enabled":
            cloned["notes"] = "Restore the initial reset when the hot-water loop is disabled."
        blocks.append(cloned)

    def hot_endpoint(endpoint):
        endpoint = endpoint.replace("chilled_reset_", "hot_reset_")
        endpoint = endpoint.replace("chilled_water_pump_needed", "hot_water_pump_needed")
        endpoint = endpoint.replace("chilled_water_supply_setpoint", "hot_water_supply_setpoint")
        endpoint = endpoint.replace("chilled_water_reset_fraction", "hot_water_reset_fraction")
        endpoint = endpoint.replace("chilled_water_source_error.u2", "hot_water_source_error.u1")
        for ahu in (1, 2):
            endpoint = endpoint.replace(f"ahu_{ahu}_select_three_requests", f"ahu_{ahu}_hot_select_three_requests")
        return endpoint

    for edge in cold_edges:
        if edge["from"].startswith("chilled_reset_") or edge["to"].startswith("chilled_reset_"):
            edges.append({"from": hot_endpoint(edge["from"]), "to": hot_endpoint(edge["to"])})
    edges[:] = [edge for edge in edges if not (edge["from"] == "hot_water_supply_setpoint.y" and
                                                 edge["to"] in {"hot_water_supply_setpoint_c", "hot_water_source_error.u1"})]
    plant["outputs"].append(port("hot_water_reset_fraction", unit="fraction (0–1)",
                                 label="Hot Water Plant Reset", brick="Point"))
    graph["outputs"].append(port("plant_hot_water_reset_fraction", unit="fraction (0–1)",
                                 label="Plant Hot Water Reset", brick="Point"))
    graph["connections"].append({"from": "plant.hot_water_reset_fraction", "to": "plant_hot_water_reset_fraction"})
    plant["notes"] += " AHU hot-water requests reset the supply target every five minutes between 44 and 50 °C."


add_hot_water_reset()


def add_terminal_hot_water_requests():
    """Aggregate persistent VAV reheat shortfalls into the plant hot-water reset."""
    plant = next(block for block in graph["blocks"] if block["name"] == "plant")
    blocks, edges = plant["blocks"], plant["connections"]
    def wire(source, target):
        edges.append({"from": source, "to": target})
    blocks.append(constant("terminal_heat_demand_threshold", 0.5, "fraction (0–1)",
                           "Request hotter water when zone heating error exceeds one kelvin."))
    requests = []
    for index in range(1, 11):
        name = f"zone_{index:02}"
        for point, brick in (("reheat_valve", "Valve_Position_Command"), ("heating_demand", "Point")):
            input_name = f"{name}_{point}"
            plant["inputs"].append(port(input_name, unit="fraction (0–1)",
                                        label=f"Zone {index} {point.replace('_', ' ').title()}", brick=brick))
            output_name = "reheat_valve_position" if point == "reheat_valve" else point
            graph["connections"].append({"from": f"{name}.{output_name}", "to": f"plant.{input_name}"})
        blocks.extend([
            {"name": f"{name}_hot_valve_request", "block_type": "Hysteresis", "category": "elementary",
             "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "uLow", "value": 0.35}, {"name": "uHigh", "value": 0.5}],
             "notes": "VAV reheat valve must exceed 50%; request clears below 35%."},
            binary(f"{name}_hot_demand_request", "Greater", note="Zone heating demand exceeds the editable one-kelvin-equivalent threshold."),
            binary(f"{name}_hot_request_condition", "And", "Boolean", note="Both reheat valve and zone heating deficit call for hotter water."),
            {"name": f"{name}_hot_request_delay", "block_type": "TrueDelay", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "delayTime", "value": 300.0, "unit": "s"},
                            {"name": "delayOnInit", "type": "Boolean", "value": True}],
             "notes": "Require five continuous minutes of terminal reheat shortfall."},
            {"name": f"{name}_hot_request_count", "block_type": "BooleanToInteger", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Integer", "count")],
             "notes": "One hot-water plant request from this VAV."},
        ])
        for source, target in [
            (f"{name}_reheat_valve", f"{name}_hot_valve_request.u"),
            (f"{name}_heating_demand", f"{name}_hot_demand_request.u1"),
            ("terminal_heat_demand_threshold.y", f"{name}_hot_demand_request.u2"),
            (f"{name}_hot_valve_request.y", f"{name}_hot_request_condition.u1"),
            (f"{name}_hot_demand_request.y", f"{name}_hot_request_condition.u2"),
            (f"{name}_hot_request_condition.y", f"{name}_hot_request_delay.u"),
            (f"{name}_hot_request_delay.y", f"{name}_hot_request_count.u"),
        ]:
            wire(source, target)
        requests.append(f"{name}_hot_request_count.y")
    level = 0
    while len(requests) > 1:
        following = []
        for pair_index in range(0, len(requests), 2):
            if pair_index + 1 == len(requests):
                following.append(requests[pair_index])
                continue
            name = f"terminal_hot_sum_{level}_{pair_index // 2}"
            blocks.append({"name": name, "block_type": "Buildings.Controls.OBC.CDL.Integers.Add",
                           "category": "elementary", "inputs": [port("u1", "Integer", "count"), port("u2", "Integer", "count")],
                           "outputs": [port("y", "Integer", "count")],
                           "notes": "Sum VAV hot-water reset requests without a simulator-side decision."})
            wire(requests[pair_index], f"{name}.u1")
            wire(requests[pair_index + 1], f"{name}.u2")
            following.append(f"{name}.y")
        requests = following
        level += 1
    blocks.append({"name": "hot_reset_all_request_count", "block_type": "Buildings.Controls.OBC.CDL.Integers.Add",
                   "category": "elementary", "inputs": [port("u1", "Integer", "count"), port("u2", "Integer", "count")],
                   "outputs": [port("y", "Integer", "count")],
                   "notes": "Sum AHU hot-water requests and persistent terminal reheat requests."})
    wire("hot_reset_request_count.y", "hot_reset_all_request_count.u1")
    wire(requests[0], "hot_reset_all_request_count.u2")
    wire(requests[0], "terminal_hot_water_request_count")
    wire("hot_reset_all_request_count.y", "hot_reset_request_real.u")
    edges[:] = [edge for edge in edges if not (edge["from"] == "hot_reset_request_count.y" and
                                                 edge["to"] == "hot_reset_request_real.u")]
    plant["outputs"].append(port("terminal_hot_water_request_count", "Integer", "count",
                                 "Terminal Hot Water Reset Requests", "Point"))
    graph["outputs"].append(port("plant_terminal_hot_water_request_count", "Integer", "count",
                                 "Plant Terminal Hot Water Reset Requests", "Point"))
    graph["connections"].append({"from": "plant.terminal_hot_water_request_count",
                                  "to": "plant_terminal_hot_water_request_count"})
    plant["notes"] += " Persistent VAV reheat demand contributes requests to the hot-water reset."


add_terminal_hot_water_requests()


def add_supply_air_temperature_reset():
    """Use qualified VAV cooling requests to reset each AHU supply target."""
    for ahu in (1, 2):
        name = f"ahu_{ahu}"
        controller = next(block for block in graph["blocks"] if block["name"] == name)
        blocks, edges = controller["blocks"], controller["connections"]
        def wire(source, target):
            edges.append({"from": source, "to": target})
        requests = []
        for zone_index in range((ahu - 1) * 5 + 1, ahu * 5 + 1):
            zone = f"zone_{zone_index:02}"
            for signal in ("cooling_demand", "damper_position"):
                controller["inputs"].append(port(f"{zone}_{signal}", unit="fraction (0–1)",
                                                 label=f"Zone {zone_index} {signal.replace('_', ' ').title()}",
                                                 brick="Damper_Position_Command" if signal == "damper_position" else "Point"))
                graph["connections"].append({"from": f"{zone}.{signal}", "to": f"{name}.{zone}_{signal}"})
            blocks.extend([
                constant(f"{zone}_sat_demand_threshold", 0.5, "fraction (0–1)",
                         "One kelvin of cooling error qualifies this VAV's supply-temperature request."),
                binary(f"{zone}_sat_demand_high", "Greater", note="Zone cooling demand exceeds the editable threshold."),
                {"name": f"{zone}_sat_damper_open", "block_type": "Hysteresis", "category": "elementary",
                 "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", "Boolean")],
                 "parameters": [{"name": "uLow", "value": 0.85}, {"name": "uHigh", "value": 0.95}],
                 "notes": "Request a colder supply target when the VAV damper exceeds 95%; clear below 85%."},
                binary(f"{zone}_sat_request_condition", "And", "Boolean", note="Both cooling error and open damper request a colder AHU target."),
                {"name": f"{zone}_sat_request_delay", "block_type": "TrueDelay", "category": "elementary",
                 "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
                 "parameters": [{"name": "delayTime", "value": 120.0, "unit": "s"},
                                {"name": "delayOnInit", "type": "Boolean", "value": True}],
                 "notes": "Require two continuous minutes of zone cooling request."},
                {"name": f"{zone}_sat_request_count", "block_type": "BooleanToInteger", "category": "elementary",
                 "inputs": [port("u", "Boolean")], "outputs": [port("y", "Integer", "count")],
                 "notes": "One cooling supply-air temperature reset request from this VAV."},
            ])
            for source, target in [
                (f"{zone}_cooling_demand", f"{zone}_sat_demand_high.u1"),
                (f"{zone}_sat_demand_threshold.y", f"{zone}_sat_demand_high.u2"),
                (f"{zone}_damper_position", f"{zone}_sat_damper_open.u"),
                (f"{zone}_sat_demand_high.y", f"{zone}_sat_request_condition.u1"),
                (f"{zone}_sat_damper_open.y", f"{zone}_sat_request_condition.u2"),
                (f"{zone}_sat_request_condition.y", f"{zone}_sat_request_delay.u"),
                (f"{zone}_sat_request_delay.y", f"{zone}_sat_request_count.u"),
            ]:
                wire(source, target)
            requests.append(f"{zone}_sat_request_count.y")
        for index in range(4):
            name_sum = f"sat_request_sum_{index + 1}"
            blocks.append({"name": name_sum, "block_type": "Buildings.Controls.OBC.CDL.Integers.Add",
                           "category": "elementary", "inputs": [port("u1", "Integer", "count"), port("u2", "Integer", "count")],
                           "outputs": [port("y", "Integer", "count")], "notes": "Sum VAV cooling reset requests."})
            wire(requests[0] if index == 0 else f"sat_request_sum_{index}.y", f"{name_sum}.u1")
            wire(requests[index + 1], f"{name_sum}.u2")
        blocks.extend([
            {"name": "sat_request_real", "block_type": "IntegerToReal", "category": "elementary",
             "inputs": [port("u", "Integer", "count")], "outputs": [port("y", unit="count")],
             "notes": "Convert the VAV request count for sampled reset arithmetic."},
            constant("sat_ignored_requests", 2.0, "count", "Ignore the first two zone requests."),
            binary("sat_excess_requests", "Subtract", "Real", "Real", "Requests above the ignored count."),
            constant("sat_zero_requests", 0.0, "count", "Lower bound for excess requests."),
            binary("sat_positive_requests", "Max", "Real", "Real", "Only respond to requests above the ignored count."),
            {"name": "sat_response_gain", "block_type": "MultiplyByParameter", "category": "elementary",
             "inputs": [port("u", unit="count")], "outputs": [port("y", unit="K")],
             "parameters": [{"name": "k", "type": "Real", "value": -0.2, "unit": "K/request"}],
             "notes": "Lower the supply target by 0.2 K per excess request each sample."},
            constant("sat_max_response", -0.6, "K", "Cap each sample's cooling response at 0.6 K."),
            binary("sat_bounded_response", "Max", "Real", "Real", "Limit the negative temperature response."),
            constant("sat_trim", 0.1, "K", "Warm the target by 0.1 K each sample without enough requests."),
            binary("sat_delta", "Add", "Real", "Real", "Combine the warm trim and cool response."),
            {"name": "sat_previous", "block_type": "Buildings.Controls.OBC.CDL.Discrete.UnitDelay", "category": "elementary",
             "inputs": [port("u", unit="°C")], "outputs": [port("y", unit="°C")],
             "parameters": [{"name": "samplePeriod", "value": 120.0, "unit": "s"}, {"name": "y_start", "value": 16.0}],
             "notes": "Hold the previous two-minute supply-air temperature target."},
            binary("sat_target_sum", "Add", "Real", "Real", "Apply the sampled reset to the previous supply target."),
            constant("sat_minimum", 14.0, "°C", "Minimum supply-air target for this simplified coil model."),
            binary("sat_above_minimum", "Max", "Real", "Real", "Do not cool the supply target below 14 °C."),
            binary("sat_below_weather_max", "Min", "Real", "Real", "Use the existing outdoor-temperature target as the adjustable upper bound."),
            {"name": "sat_fan_delay", "block_type": "TrueDelay", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "delayTime", "value": 600.0, "unit": "s"},
                            {"name": "delayOnInit", "type": "Boolean", "value": True}],
             "notes": "Hold the outdoor-temperature target for ten minutes after fan demand starts."},
            switch("sat_candidate", "°C", "Select the reset target after fan startup."),
            switch("sat_output", "°C", "Use the sampled target while the fan is active; otherwise use the weather target."),
        ])
        for source, target in [
            ("sat_request_sum_4.y", "sat_request_count"),
            ("sat_request_sum_4.y", "sat_request_real.u"),
            ("sat_request_real.y", "sat_excess_requests.u1"),
            ("sat_ignored_requests.y", "sat_excess_requests.u2"),
            ("sat_excess_requests.y", "sat_positive_requests.u1"),
            ("sat_zero_requests.y", "sat_positive_requests.u2"),
            ("sat_positive_requests.y", "sat_response_gain.u"),
            ("sat_response_gain.y", "sat_bounded_response.u1"),
            ("sat_max_response.y", "sat_bounded_response.u2"),
            ("sat_bounded_response.y", "sat_delta.u1"),
            ("sat_trim.y", "sat_delta.u2"),
            ("sat_previous.y", "sat_target_sum.u1"),
            ("sat_delta.y", "sat_target_sum.u2"),
            ("sat_target_sum.y", "sat_above_minimum.u1"),
            ("sat_minimum.y", "sat_above_minimum.u2"),
            ("sat_above_minimum.y", "sat_below_weather_max.u1"),
            ("supply_reset.y", "sat_below_weather_max.u2"),
            ("sat_below_weather_max.y", "sat_candidate.u1"),
            ("supply_reset.y", "sat_candidate.u2"),
            ("sat_fan_delay.y", "sat_candidate.u3"),
            ("sat_candidate.y", "sat_previous.u"),
            ("sat_previous.y", "sat_output.u1"),
            ("supply_reset.y", "sat_output.u2"),
            ("sat_fan_delay.y", "sat_output.u3"),
            ("fan_requested.y", "sat_fan_delay.u"),
        ]:
            wire(source, target)
        for edge in edges:
            if edge["from"] == "supply_reset.y" and edge["to"] not in {
                "sat_below_weather_max.u2", "sat_candidate.u2", "sat_output.u2"
            }:
                edge["from"] = "sat_output.y"
        controller["outputs"].append(port("sat_request_count", "Integer", "count",
                                           "Cooling Supply Air Temperature Requests", "Point"))
        graph["outputs"].append(port(f"ahu_{ahu}_sat_request_count", "Integer", "count",
                                      f"AHU {ahu} Cooling Supply Air Temperature Requests", "Point"))
        graph["connections"].append({"from": f"ahu_{ahu}.sat_request_count",
                                      "to": f"ahu_{ahu}_sat_request_count"})
        controller["notes"] += " VAV cooling requests reset the sampled supply-air target within its outdoor-temperature bound."


add_supply_air_temperature_reset()


def add_terminal_discharge_control():
    """Reset heating discharge temperature and close the valve loop in CDL."""
    for index in range(1, 11):
        name = f"zone_{index:02}"
        zone = next(block for block in graph["blocks"] if block["name"] == name)
        blocks, edges = zone["blocks"], zone["connections"]
        blocks.extend([
            constant("discharge_max_rise", 11.0, "K", "Maximum heating discharge target above the active zone heating setpoint."),
            binary("discharge_upper", "Add", "Real", "Real", "Active heating setpoint plus the editable maximum rise."),
            binary("discharge_span", "Subtract", "Real", "Real", "Difference from AHU supply target to maximum heating discharge target."),
            constant("discharge_zero_rise", 0.0, "K", "Do not request a negative reheat span."),
            binary("discharge_positive_span", "Max", "Real", "Real", "Nonnegative discharge temperature rise available for heating."),
            {"name": "discharge_demand_double", "block_type": "MultiplyByParameter", "category": "elementary",
             "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", unit="fraction (0–1)")],
             "parameters": [{"name": "k", "type": "Real", "value": 2.0}],
             "notes": "Reach maximum discharge target at 50% heating-loop demand."},
            {"name": "discharge_demand_limit", "block_type": "Limiter", "category": "elementary",
             "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", unit="fraction (0–1)")],
             "parameters": [{"name": "uMin", "value": 0.0}, {"name": "uMax", "value": 1.0}],
             "notes": "Keep discharge reset fraction between zero and one."},
            binary("discharge_reset_rise", "Multiply", "Real", "Real", "Scale available temperature rise by heating-loop demand."),
            binary("discharge_reset_target", "Add", "Real", "Real", "Reset target from AHU supply temperature to active heating setpoint plus 11 K."),
            binary("discharge_target_cap", "Min", "Real", "Real", "Apply the editable absolute discharge temperature upper bound."),
            constant("discharge_minimum", 10.0, "°C", "Minimum discharge temperature for the low-temperature reheat exception."),
            switch("discharge_target", "°C", "Use heating reset target in heating mode; otherwise protect the 10 °C minimum."),
            {"name": "discharge_valve_pi", "block_type": "LimPID", "category": "elementary",
             "inputs": [port("u_s", unit="°C"), port("u_m", unit="°C")],
             "outputs": [port("y", unit="fraction (0–1)")],
             "parameters": [{"name": "k", "value": 0.05, "unit": "K⁻¹"}, {"name": "Ti", "value": 900.0, "unit": "s"},
                            {"name": "yMin", "value": 0.0}, {"name": "yMax", "value": 1.0},
                            {"name": "reverseActing", "type": "Boolean", "value": True}],
             "notes": "Modulate the reheat valve from measured discharge temperature; gain is tuned to the simplified 8 kW coil."},
            binary("discharge_below_minimum", "Less", note="Measured discharge is below the 10 °C protection minimum."),
            binary("discharge_valve_enable", "Or", "Boolean", "Boolean", "Enable reheat during a heating call or a low discharge temperature."),
            constant("discharge_valve_closed", 0.0, "fraction (0–1)", "Close reheat valve outside heating and low-temperature protection."),
            switch("discharge_valve_select", "fraction (0–1)", "Gate the discharge-temperature PI valve command."),
        ])
        unit_map = {
            "discharge_upper": (("°C", "K"), "°C"),
            "discharge_span": (("°C", "°C"), "K"),
            "discharge_positive_span": (("K", "K"), "K"),
            "discharge_reset_rise": (("K", "fraction (0–1)"), "K"),
            "discharge_reset_target": (("°C", "K"), "°C"),
            "discharge_target_cap": (("°C", "°C"), "°C"),
            "discharge_below_minimum": (("°C", "°C"), "Boolean"),
            "discharge_valve_enable": (("Boolean", "Boolean"), "Boolean"),
        }
        for block in blocks:
            if block["name"] in unit_map:
                input_units, output_unit = unit_map[block["name"]]
                for connector, unit in zip(block["inputs"], input_units):
                    connector["unit"] = unit
                block["outputs"][0]["unit"] = output_unit
        edges.extend({"from": source, "to": target} for source, target in [
            ("heat_setpoint_active.y", "discharge_upper.u1"),
            ("discharge_max_rise.y", "discharge_upper.u2"),
            ("discharge_upper.y", "discharge_span.u1"),
            ("supply_air_temp", "discharge_span.u2"),
            ("discharge_span.y", "discharge_positive_span.u1"),
            ("discharge_zero_rise.y", "discharge_positive_span.u2"),
            ("heat_pi.y", "discharge_demand_double.u"),
            ("discharge_demand_double.y", "discharge_demand_limit.u"),
            ("discharge_positive_span.y", "discharge_reset_rise.u1"),
            ("discharge_demand_limit.y", "discharge_reset_rise.u2"),
            ("supply_air_temp", "discharge_reset_target.u1"),
            ("discharge_reset_rise.y", "discharge_reset_target.u2"),
            ("discharge_reset_target.y", "discharge_target_cap.u1"),
            ("maximum_discharge_temp.y", "discharge_target_cap.u2"),
            ("discharge_target_cap.y", "discharge_target.u1"),
            ("discharge_minimum.y", "discharge_target.u2"),
            ("heating_mode.y", "discharge_target.u3"),
            ("discharge_target.y", "discharge_valve_pi.u_s"),
            ("discharge_temp", "discharge_valve_pi.u_m"),
            ("discharge_temp", "discharge_below_minimum.u1"),
            ("discharge_minimum.y", "discharge_below_minimum.u2"),
            ("heating_mode.y", "discharge_valve_enable.u1"),
            ("discharge_below_minimum.y", "discharge_valve_enable.u2"),
            ("discharge_valve_pi.y", "discharge_valve_select.u1"),
            ("discharge_valve_closed.y", "discharge_valve_select.u2"),
            ("discharge_valve_enable.y", "discharge_valve_select.u3"),
            ("discharge_target.y", "discharge_air_setpoint_c"),
        ])
        for edge in edges:
            if edge["from"] == "reheat_select.y" and edge["to"] == "reheat_capped.u1":
                edge["from"] = "discharge_valve_select.y"
        zone["outputs"].append(port("discharge_air_setpoint_c", unit="°C", label="Discharge Air Temperature Setpoint",
                                    brick="Discharge_Air_Temperature_Setpoint"))
        graph["outputs"].append(port(f"{name}_discharge_air_setpoint_c", unit="°C",
                                     label=f"Zone {index} Discharge Air Temperature Setpoint",
                                     brick="Discharge_Air_Temperature_Setpoint"))
        graph["connections"].append({"from": f"{name}.discharge_air_setpoint_c",
                                      "to": f"{name}_discharge_air_setpoint_c"})
        zone["notes"] += " Heating demand resets the discharge target; a PI loop uses measured discharge temperature to command reheat."


add_terminal_discharge_control()


def add_terminal_heating_airflow_interlock():
    """Only increase heating airflow after the discharge air can warm the zone."""
    for index in range(1, 11):
        zone = next(block for block in graph["blocks"] if block["name"] == f"zone_{index:02}")
        blocks, edges = zone["blocks"], zone["connections"]
        blocks.extend([
            constant("heating_airflow_midpoint", 0.5, "fraction (0–1)", "The first half of heating demand resets discharge temperature, not airflow."),
            binary("heating_airflow_excess", "Subtract", "Real", "Real", "Heating demand above the 50% discharge-reset stage."),
            {"name": "heating_airflow_double", "block_type": "MultiplyByParameter", "category": "elementary",
             "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", unit="fraction (0–1)")],
             "parameters": [{"name": "k", "value": 2.0, "type": "Real"}],
             "notes": "Map the upper half of heating demand to the full heating airflow range."},
            {"name": "heating_airflow_limit", "block_type": "Limiter", "category": "elementary",
             "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", unit="fraction (0–1)")],
             "parameters": [{"name": "uMin", "value": 0.0}, {"name": "uMax", "value": 1.0}],
             "notes": "Keep the upper-stage heating airflow fraction in its valid range."},
            constant("heating_airflow_margin", 3.0, "K", "Only increase heating flow when discharge exceeds room temperature by 3 K."),
            binary("heating_airflow_threshold", "Add", "Real", "Real", "Room temperature plus the editable warm-discharge margin."),
            binary("heating_airflow_warm", "Greater", note="Measured discharge air is warm enough to increase primary flow."),
            binary("heating_airflow_enabled", "And", "Boolean", "Boolean", "Only raise airflow in heating mode with warm discharge air."),
            constant("heating_airflow_zero", 0.0, "fraction (0–1)", "Hold heating airflow at its active minimum until the warm-discharge condition is met."),
            switch("heating_airflow_select", "fraction (0–1)", "Apply the upper-stage heating airflow request only after discharge is warm."),
            constant("heating_warmup_full", 1.0, "fraction (0–1)", "Use the heating airflow maximum during preconditioning warm-up."),
            switch("heating_warmup_airflow", "fraction (0–1)", "In preconditioning, use the heating maximum instead of the occupied-stage airflow request."),
        ])
        unit_map = {
            "heating_airflow_excess": (("fraction (0–1)", "fraction (0–1)"), "fraction (0–1)"),
            "heating_airflow_threshold": (("°C", "K"), "°C"),
            "heating_airflow_warm": (("°C", "°C"), "Boolean"),
            "heating_airflow_enabled": (("Boolean", "Boolean"), "Boolean"),
        }
        for block in blocks:
            if block["name"] in unit_map:
                input_units, output_unit = unit_map[block["name"]]
                for connector, unit in zip(block["inputs"], input_units):
                    connector["unit"] = unit
                block["outputs"][0]["unit"] = output_unit
        edges.extend({"from": source, "to": target} for source, target in [
            ("heat_pi.y", "heating_airflow_excess.u1"),
            ("heating_airflow_midpoint.y", "heating_airflow_excess.u2"),
            ("heating_airflow_excess.y", "heating_airflow_double.u"),
            ("heating_airflow_double.y", "heating_airflow_limit.u"),
            ("room_temp", "heating_airflow_threshold.u1"),
            ("heating_airflow_margin.y", "heating_airflow_threshold.u2"),
            ("discharge_temp", "heating_airflow_warm.u1"),
            ("heating_airflow_threshold.y", "heating_airflow_warm.u2"),
            ("heating_mode.y", "heating_airflow_enabled.u1"),
            ("heating_airflow_warm.y", "heating_airflow_enabled.u2"),
            ("heating_airflow_limit.y", "heating_airflow_select.u1"),
            ("heating_airflow_zero.y", "heating_airflow_select.u2"),
            ("heating_airflow_enabled.y", "heating_airflow_select.u3"),
            ("heating_warmup_full.y", "heating_warmup_airflow.u1"),
            ("heating_airflow_select.y", "heating_warmup_airflow.u2"),
            ("preconditioning_mode", "heating_warmup_airflow.u3"),
        ])
        for edge in edges:
            if edge["from"] == "heat_pi.y" and edge["to"] == "heating_damper_gain.u":
                edge["from"] = "heating_warmup_airflow.y"
        zone["notes"] += " Heating airflow rises only after the discharge is 3 K warmer than the room."


add_terminal_heating_airflow_interlock()


def add_terminal_damper_feedback():
    """Modulate damper position from measured primary airflow and the CDL setpoint."""
    for index in range(1, 11):
        zone = next(block for block in graph["blocks"] if block["name"] == f"zone_{index:02}")
        blocks, edges = zone["blocks"], zone["connections"]
        blocks.extend([
            {"name": "damper_flow_pi", "block_type": "LimPID", "category": "elementary",
             "inputs": [port("u_s", unit="normalized airflow"), port("u_m", unit="normalized airflow")],
             "outputs": [port("y", unit="fraction (0–1)")],
             "parameters": [{"name": "k", "value": 0.5}, {"name": "Ti", "value": 300.0, "unit": "s"},
                            {"name": "yMin", "value": 0.0}, {"name": "yMax", "value": 1.0},
                            {"name": "reverseActing", "type": "Boolean", "value": True}],
             "notes": "Modulate VAV damper to track measured primary airflow against its editable setpoint."},
            constant("damper_flow_threshold", 0.01, "normalized airflow", "Close the damper when the airflow setpoint is effectively zero."),
            binary("damper_flow_requested", "Greater", note="The terminal has a nonzero primary-airflow setpoint."),
            constant("damper_closed", 0.0, "fraction (0–1)", "Command a closed damper when primary airflow is not requested."),
            switch("damper_feedback_output", "fraction (0–1)", "Use the measured-airflow PI command only while primary air is requested."),
            {"name": "measured_airflow_passthrough", "block_type": "MultiplyByParameter", "category": "elementary",
             "inputs": [port("u", unit="normalized airflow")], "outputs": [port("y", unit="normalized airflow")],
             "parameters": [{"name": "k", "value": 1.0, "type": "Real"}],
             "notes": "Expose measured primary airflow separately from its commanded setpoint."},
        ])
        block = next(item for item in blocks if item["name"] == "damper_flow_requested")
        for connector in block["inputs"]:
            connector["unit"] = "normalized airflow"
        block["outputs"][0]["unit"] = "Boolean"
        edges.extend({"from": source, "to": target} for source, target in [
            ("airflow_request_sum.y", "damper_flow_pi.u_s"),
            ("measured_airflow", "damper_flow_pi.u_m"),
            ("airflow_request_sum.y", "damper_flow_requested.u1"),
            ("damper_flow_threshold.y", "damper_flow_requested.u2"),
            ("damper_flow_pi.y", "damper_feedback_output.u1"),
            ("damper_closed.y", "damper_feedback_output.u2"),
            ("damper_flow_requested.y", "damper_feedback_output.u3"),
            ("measured_airflow", "measured_airflow_passthrough.u"),
            ("measured_airflow_passthrough.y", "airflow"),
        ])
        for edge in edges:
            if edge["from"] == "heating_damper_select.y" and edge["to"] in ("damper_position", "damper_nearly_open.u1"):
                edge["from"] = "damper_feedback_output.y"
        edges[:] = [edge for edge in edges if not (edge["from"] == "airflow_sum.y" and edge["to"] == "airflow")]
        zone["notes"] += " The damper PI tracks measured primary airflow; its setpoint remains separate from the physical delivery."


add_terminal_damper_feedback()


def add_ahu_freeze_stage_one():
    """Give both AHUs a timed, graph-owned first freeze-protection stage."""
    plant = next(block for block in graph["blocks"] if block["name"] == "plant")
    for index in (1, 2):
        name = f"ahu_{index}"
        ahu = next(block for block in graph["blocks"] if block["name"] == name)
        blocks, edges = ahu["blocks"], ahu["connections"]
        blocks.extend([
            constant("freeze_stage_one_on", 4.4, "°C", "Stage 1 begins after supply air stays below 4.4 °C for five minutes."),
            constant("freeze_stage_one_off", 7.0, "°C", "Stage 1 clears after supply air stays above 7 °C for five minutes."),
            binary("freeze_stage_one_cold", "Less", note="Detect low measured supply-air temperature."),
            binary("freeze_stage_one_warm", "Greater", note="Detect recovered measured supply-air temperature."),
            {"name": "freeze_stage_one_on_delay", "block_type": "TrueDelay", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "delayTime", "value": 300.0, "unit": "s"},
                            {"name": "delayOnInit", "type": "Boolean", "value": True}],
             "notes": "Require five continuous minutes below the Stage 1 temperature threshold."},
            {"name": "freeze_stage_one_off_delay", "block_type": "TrueDelay", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "delayTime", "value": 300.0, "unit": "s"},
                            {"name": "delayOnInit", "type": "Boolean", "value": True}],
             "notes": "Require five continuous minutes above the recovery threshold."},
            {"name": "freeze_stage_one_latch", "block_type": "Latch", "category": "elementary",
             "inputs": [port("u", "Boolean"), port("clr", "Boolean")],
             "outputs": [port("y", "Boolean")],
             "notes": "Hold Stage 1 during the supply-temperature recovery interval."},
            switch("freeze_stage_one_outdoor_air", "fraction (0–1)",
                   "Override the outdoor-air command to its editable minimum while Stage 1 is active."),
            binary("freeze_stage_one_heating_enable", "Or", "Boolean", "Boolean",
                   "Keep the heating coil enabled during Stage 1 even if normal mixed-air logic would disable it."),
            {"name": "freeze_stage_one_request_integer", "block_type": "BooleanToInteger", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Integer", "count")],
             "notes": "Convert Stage 1 status into a hot-water plant request."},
            {"name": "freeze_stage_one_two_requests", "block_type": "Buildings.Controls.OBC.CDL.Integers.Add",
             "category": "elementary", "inputs": [port("u1", "Integer", "count"), port("u2", "Integer", "count")],
             "outputs": [port("y", "Integer", "count")],
             "notes": "Issue two hot-water plant requests during Stage 1."},
        ])
        edges.extend({"from": source, "to": target} for source, target in [
            ("actual_supply_air_temp", "freeze_stage_one_cold.u1"),
            ("freeze_stage_one_on.y", "freeze_stage_one_cold.u2"),
            ("actual_supply_air_temp", "freeze_stage_one_warm.u1"),
            ("freeze_stage_one_off.y", "freeze_stage_one_warm.u2"),
            ("freeze_stage_one_cold.y", "freeze_stage_one_on_delay.u"),
            ("freeze_stage_one_warm.y", "freeze_stage_one_off_delay.u"),
            ("freeze_stage_one_on_delay.y", "freeze_stage_one_latch.u"),
            ("freeze_stage_one_off_delay.y", "freeze_stage_one_latch.clr"),
            ("minimum_outdoor_air.y", "freeze_stage_one_outdoor_air.u1"),
            ("minimum_oa_command.y", "freeze_stage_one_outdoor_air.u2"),
            ("freeze_stage_one_latch.y", "freeze_stage_one_outdoor_air.u3"),
            ("heating_coil_needed.y", "freeze_stage_one_heating_enable.u1"),
            ("freeze_stage_one_latch.y", "freeze_stage_one_heating_enable.u2"),
            ("freeze_stage_one_latch.y", "freeze_stage_one"),
            ("freeze_stage_one_latch.y", "freeze_stage_one_request_integer.u"),
            ("freeze_stage_one_request_integer.y", "freeze_stage_one_two_requests.u1"),
            ("freeze_stage_one_request_integer.y", "freeze_stage_one_two_requests.u2"),
            ("freeze_stage_one_two_requests.y", "freeze_hot_water_request"),
        ])
        for edge in edges:
            if edge["from"] == "minimum_oa_command.y" and edge["to"] == "outdoor_air_fraction":
                edge["from"] = "freeze_stage_one_outdoor_air.y"
            if edge["from"] == "heating_coil_needed.y" and edge["to"] == "heating_coil_enable.u3":
                edge["from"] = "freeze_stage_one_heating_enable.y"
        ahu["outputs"].extend([
            port("freeze_stage_one", "Boolean", "unitless", "Freeze Protection Stage 1", "Point"),
            port("freeze_hot_water_request", "Integer", "count", "Freeze Hot Water Plant Requests", "Point"),
        ])
        graph["outputs"].extend([
            port(f"{name}_freeze_stage_one", "Boolean", "unitless", f"AHU {index} Freeze Protection Stage 1", "Point"),
            port(f"{name}_freeze_hot_water_request", "Integer", "count", f"AHU {index} Freeze Hot Water Plant Requests", "Point"),
        ])
        graph["connections"].extend([
            {"from": f"{name}.freeze_stage_one", "to": f"{name}_freeze_stage_one"},
            {"from": f"{name}.freeze_hot_water_request", "to": f"{name}_freeze_hot_water_request"},
            {"from": f"{name}.freeze_hot_water_request", "to": f"plant.{name}_freeze_hot_water_request"},
        ])
        plant["inputs"].append(port(f"{name}_freeze_hot_water_request", "Integer", "count",
                                    f"AHU {index} Freeze Hot Water Plant Requests", "Point"))
        ahu["notes"] += " A timed Stage 1 freeze response reduces outdoor air to its minimum and sends two hot-water requests."

    plant["blocks"].extend([
        {"name": "freeze_hot_water_request_sum", "block_type": "Buildings.Controls.OBC.CDL.Integers.Add",
         "category": "elementary", "inputs": [port("u1", "Integer", "count"), port("u2", "Integer", "count")],
         "outputs": [port("y", "Integer", "count")],
         "notes": "Sum Stage 1 freeze requests from both AHUs."},
        {"name": "hot_reset_with_freeze_requests", "block_type": "Buildings.Controls.OBC.CDL.Integers.Add",
         "category": "elementary", "inputs": [port("u1", "Integer", "count"), port("u2", "Integer", "count")],
         "outputs": [port("y", "Integer", "count")],
         "notes": "Add freeze requests before the sampled hot-water reset."},
        {"name": "freeze_request_count_real", "block_type": "IntegerToReal", "category": "elementary",
         "inputs": [port("u", "Integer", "count")], "outputs": [port("y", unit="count")],
         "notes": "Convert freeze request count for the plant enable comparison."},
        constant("freeze_request_zero", 0.0, "count", "No active freeze protection requests."),
        binary("freeze_hot_water_active", "Greater", note="At least one AHU requests freeze protection."),
        binary("freeze_hot_water_pump_enable", "Or", "Boolean", "Boolean",
               "Run the hot-water pump when ordinary load or AHU freeze protection requires it."),
        constant("freeze_hot_water_nominal", 45.0, "°C", "Keep the hot-water target at least 45 °C during AHU freeze protection."),
        binary("freeze_hot_water_minimum_target", "Max", "Real", "Real",
               "Prevent hot-water reset from trimming below the freeze-protection target."),
        switch("freeze_hot_water_target", "°C", "Apply the freeze minimum only while an AHU requests protection."),
    ])
    plant["connections"] = [edge for edge in plant["connections"] if not (
        (edge["from"] == "hot_reset_all_request_count.y" and edge["to"] == "hot_reset_request_real.u") or
        (edge["from"] == "hot_water_pump_needed.y" and edge["to"] in
         ("hot_water_enable", "hot_water_pump_select.u3", "hot_reset_enabled.u3")) or
        (edge["from"] == "hot_reset_supply_sum.y" and edge["to"] == "hot_water_supply_setpoint_c"))]
    plant["connections"].extend({"from": source, "to": target} for source, target in [
        ("ahu_1_freeze_hot_water_request", "freeze_hot_water_request_sum.u1"),
        ("ahu_2_freeze_hot_water_request", "freeze_hot_water_request_sum.u2"),
        ("hot_reset_all_request_count.y", "hot_reset_with_freeze_requests.u1"),
        ("freeze_hot_water_request_sum.y", "hot_reset_with_freeze_requests.u2"),
        ("hot_reset_with_freeze_requests.y", "hot_reset_request_real.u"),
        ("freeze_hot_water_request_sum.y", "freeze_request_count_real.u"),
        ("freeze_request_count_real.y", "freeze_hot_water_active.u1"),
        ("freeze_request_zero.y", "freeze_hot_water_active.u2"),
        ("hot_water_pump_needed.y", "freeze_hot_water_pump_enable.u1"),
        ("freeze_hot_water_active.y", "freeze_hot_water_pump_enable.u2"),
        ("freeze_hot_water_pump_enable.y", "hot_water_enable"),
        ("freeze_hot_water_pump_enable.y", "hot_water_pump_select.u3"),
        ("freeze_hot_water_pump_enable.y", "hot_reset_enabled.u3"),
        ("hot_reset_supply_sum.y", "freeze_hot_water_minimum_target.u1"),
        ("freeze_hot_water_nominal.y", "freeze_hot_water_minimum_target.u2"),
        ("freeze_hot_water_minimum_target.y", "freeze_hot_water_target.u1"),
        ("hot_reset_supply_sum.y", "freeze_hot_water_target.u2"),
        ("freeze_hot_water_active.y", "freeze_hot_water_target.u3"),
        ("freeze_hot_water_target.y", "hot_water_supply_setpoint_c"),
    ])
    plant["notes"] += " AHU Stage 1 freeze protection contributes hot-water reset requests."


add_ahu_freeze_stage_one()


def add_ahu_freeze_stage_two():
    """Interrupt outdoor ventilation for one hour after a deeper timed freeze fault."""
    for index in (1, 2):
        name = f"ahu_{index}"
        ahu = next(block for block in graph["blocks"] if block["name"] == name)
        blocks, edges = ahu["blocks"], ahu["connections"]
        blocks.extend([
            constant("freeze_stage_two_on", 3.3, "°C", "Stage 2 begins after supply air stays below 3.3 °C for five minutes."),
            binary("freeze_stage_two_cold", "Less", note="Detect the deeper measured supply-air temperature fault."),
            {"name": "freeze_stage_two_on_delay", "block_type": "TrueDelay", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "delayTime", "value": 300.0, "unit": "s"},
                            {"name": "delayOnInit", "type": "Boolean", "value": True}],
             "notes": "Require five continuous minutes below the Stage 2 temperature threshold."},
            {"name": "freeze_stage_two_latch", "block_type": "Latch", "category": "elementary",
             "inputs": [port("u", "Boolean"), port("clr", "Boolean")],
             "outputs": [port("y", "Boolean")],
             "notes": "Hold the outdoor-air interruption until its one-hour timer has elapsed."},
            {"name": "freeze_stage_two_to_integer", "block_type": "BooleanToInteger", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Integer", "count")],
             "notes": "Convert Stage 2 status for a sampled timer feedback path."},
            {"name": "freeze_stage_two_to_real", "block_type": "IntegerToReal", "category": "elementary",
             "inputs": [port("u", "Integer", "count")], "outputs": [port("y", unit="count")],
             "notes": "Convert the sampled Stage 2 status to a Real signal for the duration comparison."},
            {"name": "freeze_stage_two_previous", "block_type": "UnitDelay", "category": "elementary",
             "inputs": [port("u", unit="count")], "outputs": [port("y", unit="count")],
             "parameters": [{"name": "samplePeriod", "value": 60.0, "unit": "s"},
                            {"name": "y_start", "value": 0}],
             "notes": "Break the Stage 2 latch/timer feedback loop using its prior sampled status."},
            constant("freeze_stage_two_zero", 0.0, "count", "Zero means the prior Stage 2 state was inactive."),
            binary("freeze_stage_two_previous_active", "Greater", note="Was Stage 2 active at the prior sample?"),
            {"name": "freeze_stage_two_hour", "block_type": "TrueDelay", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "delayTime", "value": 3495.0, "unit": "s"},
                            {"name": "delayOnInit", "type": "Boolean", "value": True}],
             "notes": "Clear Stage 2 after one hour including the sampled feedback and timer step."},
            constant("freeze_stage_two_closed", 0.0, "fraction (0–1)",
                     "Close all outdoor-air paths while minimum ventilation is interrupted."),
            switch("freeze_stage_two_outdoor_air", "fraction (0–1)",
                   "Close outdoor air during Stage 2; otherwise restore Stage 1 or normal control."),
            {"name": "freeze_stage_two_alarm_integer", "block_type": "BooleanToInteger", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Integer", "level")],
             "notes": "Convert the active ventilation interruption to alarm level one."},
            {"name": "freeze_stage_two_alarm_double", "block_type": "Buildings.Controls.OBC.CDL.Integers.Add",
             "category": "elementary", "inputs": [port("u1", "Integer", "level"), port("u2", "Integer", "level")],
             "outputs": [port("y", "Integer", "level")],
             "notes": "Build Level 2 from the active Stage 2 alarm bit."},
            {"name": "freeze_stage_two_alarm_three", "block_type": "Buildings.Controls.OBC.CDL.Integers.Add",
             "category": "elementary", "inputs": [port("u1", "Integer", "level"), port("u2", "Integer", "level")],
             "outputs": [port("y", "Integer", "level")],
             "notes": "Report Level 3 while minimum ventilation is interrupted."},
        ])
        edges.extend({"from": source, "to": target} for source, target in [
            ("actual_supply_air_temp", "freeze_stage_two_cold.u1"),
            ("freeze_stage_two_on.y", "freeze_stage_two_cold.u2"),
            ("freeze_stage_two_cold.y", "freeze_stage_two_on_delay.u"),
            ("freeze_stage_two_on_delay.y", "freeze_stage_two_latch.u"),
            ("freeze_stage_two_latch.y", "freeze_stage_two_to_integer.u"),
            ("freeze_stage_two_to_integer.y", "freeze_stage_two_to_real.u"),
            ("freeze_stage_two_to_real.y", "freeze_stage_two_previous.u"),
            ("freeze_stage_two_previous.y", "freeze_stage_two_previous_active.u1"),
            ("freeze_stage_two_zero.y", "freeze_stage_two_previous_active.u2"),
            ("freeze_stage_two_previous_active.y", "freeze_stage_two_hour.u"),
            ("freeze_stage_two_hour.y", "freeze_stage_two_latch.clr"),
            ("freeze_stage_two_closed.y", "freeze_stage_two_outdoor_air.u1"),
            ("freeze_stage_one_outdoor_air.y", "freeze_stage_two_outdoor_air.u2"),
            ("freeze_stage_two_latch.y", "freeze_stage_two_outdoor_air.u3"),
            ("freeze_stage_two_latch.y", "freeze_stage_two"),
            ("freeze_stage_two_latch.y", "freeze_stage_two_alarm_integer.u"),
            ("freeze_stage_two_alarm_integer.y", "freeze_stage_two_alarm_double.u1"),
            ("freeze_stage_two_alarm_integer.y", "freeze_stage_two_alarm_double.u2"),
            ("freeze_stage_two_alarm_double.y", "freeze_stage_two_alarm_three.u1"),
            ("freeze_stage_two_alarm_integer.y", "freeze_stage_two_alarm_three.u2"),
            ("freeze_stage_two_alarm_three.y", "freeze_ventilation_alarm_level"),
        ])
        for edge in edges:
            if edge["from"] == "freeze_stage_one_outdoor_air.y" and edge["to"] == "outdoor_air_fraction":
                edge["from"] = "freeze_stage_two_outdoor_air.y"
        ahu["outputs"].extend([
            port("freeze_stage_two", "Boolean", "unitless", "Freeze Protection Stage 2", "Point"),
            port("freeze_ventilation_alarm_level", "Integer", "level", "Freeze Ventilation Alarm Level", "Point"),
        ])
        graph["outputs"].extend([
            port(f"{name}_freeze_stage_two", "Boolean", "unitless", f"AHU {index} Freeze Protection Stage 2", "Point"),
            port(f"{name}_freeze_ventilation_alarm_level", "Integer", "level", f"AHU {index} Freeze Ventilation Alarm Level", "Point"),
        ])
        graph["connections"].extend([
            {"from": f"{name}.freeze_stage_two", "to": f"{name}_freeze_stage_two"},
            {"from": f"{name}.freeze_ventilation_alarm_level", "to": f"{name}_freeze_ventilation_alarm_level"},
        ])
        ahu["notes"] += " Stage 2 closes outdoor air for one hour and reports a Level 3 ventilation alarm."


add_ahu_freeze_stage_two()
(EXAMPLES / "four_pipe_g36.json").write_text(json.dumps(graph, indent=2) + "\n")
