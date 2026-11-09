"""Factory for the editable building control graph."""
import json


# Brick describes the role of a point; these labels are for people and the
# existing connector names remain stable identifiers in saved control graphs.
POINTS = {
    "BuildingController": {
        "outdoor_c": ("Outside Air Temperature Sensor", "Outside_Air_Temperature_Sensor", "°C"),
        "heat_request_kw": ("Heating Load Request", "Point", "kW"),
        "cool_request_kw": ("Cooling Load Request", "Point", "kW"),
        "ahu_1_flow_request": ("AHU 1 Supply Air Flow Request", "Point", "normalized airflow"),
        "ahu_2_flow_request": ("AHU 2 Supply Air Flow Request", "Point", "normalized airflow"),
        "ahu_1_actual_supply_air_temp": ("AHU 1 Supply Air Temperature Sensor", "Supply_Air_Temperature_Sensor", "°C"),
        "ahu_2_actual_supply_air_temp": ("AHU 2 Supply Air Temperature Sensor", "Supply_Air_Temperature_Sensor", "°C"),
        "ahu_1_mixed_air_temp": ("AHU 1 Mixed Air Temperature Sensor", "Mixed_Air_Temperature_Sensor", "°C"),
        "ahu_2_mixed_air_temp": ("AHU 2 Mixed Air Temperature Sensor", "Mixed_Air_Temperature_Sensor", "°C"),
        "ahu_1_supply_air_temp": ("AHU 1 Supply Air Temperature Setpoint", "Supply_Air_Temperature_Setpoint", "°C"),
        "ahu_2_supply_air_temp": ("AHU 2 Supply Air Temperature Setpoint", "Supply_Air_Temperature_Setpoint", "°C"),
        "ahu_1_fan_speed": ("AHU 1 Supply Fan Speed Command", "Fan_Speed_Command", "fraction (0–1)"),
        "ahu_2_fan_speed": ("AHU 2 Supply Fan Speed Command", "Fan_Speed_Command", "fraction (0–1)"),
        "ahu_1_outdoor_air_fraction": ("AHU 1 Outdoor Air Fraction", "Point", "fraction (0–1)"),
        "ahu_2_outdoor_air_fraction": ("AHU 2 Outdoor Air Fraction", "Point", "fraction (0–1)"),
        "ahu_1_outdoor_air_damper_position": ("AHU 1 Outdoor Air Damper Command", "Damper_Position_Command", "fraction (0–1)"),
        "ahu_2_outdoor_air_damper_position": ("AHU 2 Outdoor Air Damper Command", "Damper_Position_Command", "fraction (0–1)"),
        "ahu_1_return_air_damper_position": ("AHU 1 Return Air Damper Command", "Damper_Position_Command", "fraction (0–1)"),
        "ahu_2_return_air_damper_position": ("AHU 2 Return Air Damper Command", "Damper_Position_Command", "fraction (0–1)"),
        "ahu_1_heating_coil_valve_position": ("AHU 1 Heating Coil Valve Command", "Heating_Valve_Command", "fraction (0–1)"),
        "ahu_2_heating_coil_valve_position": ("AHU 2 Heating Coil Valve Command", "Heating_Valve_Command", "fraction (0–1)"),
        "ahu_1_cooling_coil_valve_position": ("AHU 1 Cooling Coil Valve Command", "Cooling_Valve_Command", "fraction (0–1)"),
        "ahu_2_cooling_coil_valve_position": ("AHU 2 Cooling Coil Valve Command", "Cooling_Valve_Command", "fraction (0–1)"),
        "hot_water_supply_temp_c": ("Hot Water Supply Temperature Sensor", "Hot_Water_Supply_Temperature_Sensor", "°C"),
        "hot_water_return_temp_c": ("Hot Water Return Temperature Sensor", "Hot_Water_Return_Temperature_Sensor", "°C"),
        "chilled_water_supply_temp_c": ("Chilled Water Supply Temperature Sensor", "Chilled_Water_Supply_Temperature_Sensor", "°C"),
        "chilled_water_return_temp_c": ("Chilled Water Return Temperature Sensor", "Chilled_Water_Return_Temperature_Sensor", "°C"),
        "hot_water_flow_kg_s": ("Hot Water Supply Flow Sensor", "Hot_Water_Flow_Sensor", "kg/s"),
        "chilled_water_flow_kg_s": ("Chilled Water Supply Flow Sensor", "Chilled_Water_Flow_Sensor", "kg/s"),
    },
    "VAVBoxController": {
        "room_temp": ("Zone Air Temperature Sensor", "Zone_Air_Temperature_Sensor", "°C"),
        "supply_air_temp": ("Supply Air Temperature Sensor", "Supply_Air_Temperature_Sensor", "°C"),
        "supply_pressure": ("Supply Air Static Pressure Input", "Point", "normalized pressure"),
        "damper_position": ("Damper Position Command", "Damper_Position_Command", "fraction (0–1)"),
        "reheat_valve_position": ("Reheat Valve Position Command", "Valve_Position_Command", "fraction (0–1)"),
        "airflow": ("Supply Air Flow Estimate", "Point", "normalized airflow"),
        "airflow_request": ("Supply Air Flow Request", "Point", "normalized airflow"),
        "cooling_demand": ("Cooling Demand Estimate", "Point", "fraction (0–1)"),
        "heating_demand": ("Heating Demand Estimate", "Point", "fraction (0–1)"),
    },
    "AHUController": {
        "outdoor_c": ("Outside Air Temperature Sensor", "Outside_Air_Temperature_Sensor", "°C"),
        "flow_request": ("Supply Air Flow Request", "Point", "normalized airflow"),
        "actual_supply_air_temp": ("Supply Air Temperature Sensor", "Supply_Air_Temperature_Sensor", "°C"),
        "mixed_air_temp": ("Mixed Air Temperature Sensor", "Mixed_Air_Temperature_Sensor", "°C"),
        "supply_air_temp": ("Supply Air Temperature Setpoint", "Supply_Air_Temperature_Setpoint", "°C"),
        "fan_speed": ("Supply Fan Speed Command", "Fan_Speed_Command", "fraction (0–1)"),
        "outdoor_air_fraction": ("Outdoor Air Fraction", "Point", "fraction (0–1)"),
        "outdoor_air_damper_position": ("Outdoor Air Damper Command", "Damper_Position_Command", "fraction (0–1)"),
        "return_air_damper_position": ("Return Air Damper Command", "Damper_Position_Command", "fraction (0–1)"),
        "heating_coil_valve_position": ("Heating Coil Valve Command", "Heating_Valve_Command", "fraction (0–1)"),
        "cooling_coil_valve_position": ("Cooling Coil Valve Command", "Cooling_Valve_Command", "fraction (0–1)"),
    },
    "PlantDispatch": {
        "heat_request_kw": ("Heating Load Request", "Point", "kW"),
        "cool_request_kw": ("Cooling Load Request", "Point", "kW"),
        "heat_pump_heating_kw": ("Heat Pump Heating Command", "Heating_Command", "kW"),
        "heat_pump_cooling_kw": ("Heat Pump Cooling Command", "Cooling_Command", "kW"),
        "boiler_kw": ("Boiler Heating Command", "Heating_Command", "kW"),
        "chiller_kw": ("Chiller Cooling Command", "Cooling_Command", "kW"),
        "hot_water_supply_temp_c": ("Hot Water Supply Temperature Sensor", "Hot_Water_Supply_Temperature_Sensor", "°C"),
        "hot_water_return_temp_c": ("Hot Water Return Temperature Sensor", "Hot_Water_Return_Temperature_Sensor", "°C"),
        "chilled_water_supply_temp_c": ("Chilled Water Supply Temperature Sensor", "Chilled_Water_Supply_Temperature_Sensor", "°C"),
        "chilled_water_return_temp_c": ("Chilled Water Return Temperature Sensor", "Chilled_Water_Return_Temperature_Sensor", "°C"),
        "hot_water_flow_kg_s": ("Hot Water Supply Flow Sensor", "Hot_Water_Flow_Sensor", "kg/s"),
        "chilled_water_flow_kg_s": ("Chilled Water Supply Flow Sensor", "Chilled_Water_Flow_Sensor", "kg/s"),
        "hot_water_enable": ("Hot Water System Enable Command", "Hot_Water_System_Enable_Command", "Boolean (0/1)"),
        "chilled_water_enable": ("Chilled Water System Enable Command", "Chilled_Water_System_Enable_Command", "Boolean (0/1)"),
        "hot_water_pump_speed": ("Hot Water Pump Speed Command", "Speed_Command", "fraction (0–1)"),
        "chilled_water_pump_speed": ("Chilled Water Pump Speed Command", "Speed_Command", "fraction (0–1)"),
        "hot_water_supply_setpoint_c": ("Hot Water Supply Temperature Setpoint", "Supply_Hot_Water_Temperature_Setpoint", "°C"),
        "chilled_water_supply_setpoint_c": ("Chilled Water Supply Temperature Setpoint", "Supply_Chilled_Water_Temperature_Setpoint", "°C"),
    },
}


def annotate_points(item):
    definitions = POINTS.get(item["block_type"], {})
    for port in item.get("inputs", []) + item.get("outputs", []):
        definition = definitions.get(port["name"])
        if item["block_type"] == "BuildingController" and definition is None:
            if port["name"].startswith("zone_"):
                parts = port["name"].split("_")
                number = int(parts[1])
                point = "_".join(parts[2:])
                zone_points = POINTS["VAVBoxController"]
                mapped = {"room_temp": "room_temp", "supply_pressure": "supply_pressure"}.get(point, point)
                if mapped in zone_points:
                    label, brick_class, unit = zone_points[mapped]
                    definition = (f"Zone {number} {label.removeprefix('Zone ')}", brick_class, unit)
        if item["block_type"] == "BuildingController" and port["name"].startswith("plant_"):
            mapped = POINTS["PlantDispatch"].get(port["name"][6:])
            if mapped:
                label, brick_class, unit = mapped
                definition = (f"Plant {label}", brick_class, unit)
        if definition:
            label, brick_class, unit = definition
            port.update(label=label, unit=unit, description=label)
            if brick_class:
                port["brick_class"] = brick_class
    for child in item.get("blocks", []):
        annotate_points(child)


def block(name, kind, ins=(), outs=("y",), params=None, boolean_ins=(), boolean_outs=()):
    return {
        "name": name, "block_type": kind, "category": "elementary",
        "inputs": [{"name": x, "type": "Boolean" if x in boolean_ins else "Real"} for x in ins],
        "outputs": [{"name": x, "type": "Boolean" if x in boolean_outs else "Real"} for x in outs],
        "parameters": [{"name": k, "type": "Boolean" if isinstance(v, bool) else "Real", "value": v}
                       for k, v in (params or {}).items()],
    }


def composite(name, kind, ins, outs, blocks, edges, description):
    return {
        "name": name, "block_type": kind, "category": "composite", "description": description,
        "inputs": [{"name": x, "type": "Real"} for x in ins],
        "outputs": [{"name": x, "type": "Real"} for x in outs],
        "blocks": blocks,
        "connections": [{"from": a, "to": b} for a, b in edges],
    }


def annotate(blocks, notes):
    for item in blocks:
        item["notes"] = notes.get(item["name"], "")


def vav(name, setpoint=24.0):
    b = [
        block("cool_setpoint", "Constant", params={"k": setpoint}),
        block("heat_setpoint", "Constant", params={"k": 21.0}),
        block("cool_deadband", "Constant", params={"k": 0.5}),
        block("zero", "Constant", params={"k": 0.0}),
        block("minimum_damper", "Constant", params={"k": 0.15}),
        block("minimum_airflow", "Constant", params={"k": 0.2}),
        block("cool_error", "Subtract", ("u1", "u2")),
        block("heat_error", "Subtract", ("u1", "u2")),
        block("cooling_mode", "Greater", ("u1", "u2"), boolean_outs=("y",)),
        block("heating_mode", "Greater", ("u1", "u2"), boolean_outs=("y",)),
        block("air_can_cool", "Less", ("u1", "u2"), boolean_outs=("y",)),
        block("cooling_enabled", "And", ("u1", "u2"), boolean_ins=("u1", "u2"), boolean_outs=("y",)),
        block("cool_pi", "LimPID", ("u_s", "u_m"), params={"k": 0.6, "Ti": 4500.0, "yMin": 0.0, "yMax": 0.85, "reverseActing": False}),
        block("heat_pi", "LimPID", ("u_s", "u_m"), params={"k": 0.35, "Ti": 5250.0, "yMin": 0.0, "yMax": 1.0, "reverseActing": True}),
        block("damper_offset", "Add", ("u1", "u2")),
        block("damper_limit", "Limiter", ("u",), params={"uMin": 0.15, "uMax": 1.0}),
        block("damper_select", "Switch", ("u1", "u2", "u3"), boolean_ins=("u3",)),
        block("reheat_select", "Switch", ("u1", "u2", "u3"), boolean_ins=("u3",)),
        block("flow_span", "MultiplyByParameter", ("u",), params={"k": 0.75}),
        block("pressure_effect", "Multiply", ("u1", "u2")),
        block("airflow_sum", "Add", ("u1", "u2")),
        block("airflow_request_sum", "Add", ("u1", "u2")),
        block("cool_demand_gain", "MultiplyByParameter", ("u",), params={"k": 0.5}),
        block("heat_demand_gain", "MultiplyByParameter", ("u",), params={"k": 0.5}),
        block("cool_demand_limit", "Limiter", ("u",), params={"uMin": 0.0, "uMax": 1.0}),
        block("heat_demand_limit", "Limiter", ("u",), params={"uMin": 0.0, "uMax": 1.0}),
    ]
    annotate(b, {
        "cool_setpoint": "Zone cooling target. Perimeter zones use 23.5 °C; the other zones use 24 °C.",
        "heat_setpoint": "Heating target is 21 °C, leaving a deadband between heating and cooling.",
        "cool_deadband": "Cooling starts only after the room exceeds its setpoint by 0.5 °C.",
        "zero": "Shared zero signal for disabled reheat and the heating-mode comparison.",
        "minimum_damper": "Keeps the damper at least 15% open for ventilation.",
        "minimum_airflow": "Base airflow estimate before pressure and damper effects are added.",
        "cool_error": "Positive when the room is warmer than the cooling setpoint.",
        "heat_error": "Positive when the room is colder than the heating setpoint.",
        "cooling_mode": "Checks whether cooling error exceeds the 0.5 °C deadband.",
        "heating_mode": "Enables reheat only when room temperature is below the heating setpoint.",
        "air_can_cool": "Checks that supply air is cooler than the room before opening for cooling.",
        "cooling_enabled": "Both a warm room and useful cool supply air are required to open the damper.",
        "cool_pi": "PI loop turns room temperature error into a damper increment, capped at 85%.",
        "heat_pi": "Reverse-acting PI loop turns low room temperature into a reheat command.",
        "damper_offset": "Adds the minimum ventilation position to the cooling PI command.",
        "damper_limit": "Clamps the requested damper position to 15–100%.",
        "damper_select": "Uses the cooling damper request when cooling is enabled; otherwise holds minimum ventilation.",
        "reheat_select": "Passes the heating PI command only in heating mode; otherwise closes reheat.",
        "flow_span": "Scales the damper command into the variable part of the airflow estimate.",
        "pressure_effect": "Reduces estimated flow when duct pressure is low.",
        "airflow_sum": "Adds minimum airflow to the pressure-adjusted variable flow.",
        "airflow_request_sum": "Adds minimum airflow to the damper request before pressure effects; this demand drives the AHU fan.",
        "cool_demand_gain": "Converts positive cooling error into a normalized plant request.",
        "heat_demand_gain": "Converts positive heating error into a normalized plant request.",
        "cool_demand_limit": "Keeps zone cooling demand between zero and one.",
        "heat_demand_limit": "Keeps zone heating demand between zero and one.",
    })
    e = [
        ("room_temp", "cool_error.u1"), ("cool_setpoint.y", "cool_error.u2"),
        ("heat_setpoint.y", "heat_error.u1"), ("room_temp", "heat_error.u2"),
        ("cool_error.y", "cooling_mode.u1"), ("cool_deadband.y", "cooling_mode.u2"),
        ("heat_error.y", "heating_mode.u1"), ("zero.y", "heating_mode.u2"),
        ("supply_air_temp", "air_can_cool.u1"), ("room_temp", "air_can_cool.u2"),
        ("cooling_mode.y", "cooling_enabled.u1"), ("air_can_cool.y", "cooling_enabled.u2"),
        ("cool_setpoint.y", "cool_pi.u_s"), ("room_temp", "cool_pi.u_m"),
        ("heat_setpoint.y", "heat_pi.u_s"), ("room_temp", "heat_pi.u_m"),
        ("minimum_damper.y", "damper_offset.u1"), ("cool_pi.y", "damper_offset.u2"),
        ("damper_offset.y", "damper_limit.u"),
        ("damper_limit.y", "damper_select.u1"), ("minimum_damper.y", "damper_select.u2"),
        ("cooling_enabled.y", "damper_select.u3"),
        ("heat_pi.y", "reheat_select.u1"), ("zero.y", "reheat_select.u2"),
        ("heating_mode.y", "reheat_select.u3"),
        ("damper_select.y", "flow_span.u"), ("flow_span.y", "pressure_effect.u1"),
        ("supply_pressure", "pressure_effect.u2"),
        ("minimum_airflow.y", "airflow_sum.u1"), ("pressure_effect.y", "airflow_sum.u2"),
        ("minimum_airflow.y", "airflow_request_sum.u1"), ("flow_span.y", "airflow_request_sum.u2"),
        ("cool_error.y", "cool_demand_gain.u"), ("cool_demand_gain.y", "cool_demand_limit.u"),
        ("heat_error.y", "heat_demand_gain.u"), ("heat_demand_gain.y", "heat_demand_limit.u"),
        ("damper_select.y", "damper_position"), ("reheat_select.y", "reheat_valve_position"),
        ("airflow_sum.y", "airflow"), ("airflow_request_sum.y", "airflow_request"),
        ("cool_demand_limit.y", "cooling_demand"),
        ("heat_demand_limit.y", "heating_demand"),
    ]
    result = composite(name, "VAVBoxController", ("room_temp", "supply_air_temp", "supply_pressure"),
                     ("damper_position", "reheat_valve_position", "airflow", "airflow_request", "cooling_demand", "heating_demand"),
                     b, e, "VAV damper, reheat, airflow and demand composed from CDL blocks.")
    result["notes"] = "This zone controller compares room temperature with heating and cooling targets, then commands the damper and reheat valve. Open it to follow the signal path."
    annotate_points(result)
    return result


def ahu(name):
    b = [
        block("cold_threshold", "Constant", params={"k": 12.0}),
        block("warm_threshold", "Constant", params={"k": 25.0}),
        block("warmup_supply", "Constant", params={"k": 18.0}),
        block("mild_supply", "Constant", params={"k": 16.0}),
        block("cooling_supply", "Constant", params={"k": 14.0}),
        block("is_cold", "Less", ("u1", "u2"), boolean_outs=("y",)),
        block("is_mild", "Less", ("u1", "u2"), boolean_outs=("y",)),
        block("mild_or_cool", "Switch", ("u1", "u2", "u3"), boolean_ins=("u3",)),
        block("supply_reset", "Switch", ("u1", "u2", "u3"), boolean_ins=("u3",)),
        block("fan_gain", "MultiplyByParameter", ("u",), params={"k": 0.22}),
        block("fan_limit", "Limiter", ("u",), params={"uMin": 0.15, "uMax": 1.0}),
        block("minimum_outdoor_air", "Constant", params={"k": 0.2}),
        block("conventional_return_one", "Constant", params={"k": 1.0}),
        block("conventional_return_complement", "Subtract", ("u1", "u2")),
        block("coil_zero", "Constant", params={"k": 0.0}),
        block("heating_coil_error", "Subtract", ("u1", "u2")),
        block("heating_coil_feedforward", "MultiplyByParameter", ("u",), params={"k": 0.05}),
        block("heating_coil_feedback", "LimPID", ("u_s", "u_m"), params={"k": 0.015, "Ti": 1800.0, "yMin": -0.25, "yMax": 0.25, "reverseActing": True}),
        block("heating_coil_sum", "Add", ("u1", "u2")),
        block("heating_coil_limit", "Limiter", ("u",), params={"uMin": 0.0, "uMax": 1.0}),
        block("heating_coil_needed", "Less", ("u1", "u2"), boolean_outs=("y",)),
        block("heating_coil_enable", "Switch", ("u1", "u2", "u3"), boolean_ins=("u3",)),
        block("cooling_coil_error", "Subtract", ("u1", "u2")),
        block("cooling_coil_feedforward", "MultiplyByParameter", ("u",), params={"k": 0.05}),
        block("cooling_coil_feedback", "LimPID", ("u_s", "u_m"), params={"k": 0.015, "Ti": 1800.0, "yMin": -0.25, "yMax": 0.25, "reverseActing": False}),
        block("cooling_coil_sum", "Add", ("u1", "u2")),
        block("cooling_coil_limit", "Limiter", ("u",), params={"uMin": 0.0, "uMax": 1.0}),
        block("cooling_coil_needed", "Greater", ("u1", "u2"), boolean_outs=("y",)),
        block("cooling_coil_enable", "Switch", ("u1", "u2", "u3"), boolean_ins=("u3",)),
    ]
    annotate(b, {
        "cold_threshold": "Below 12 °C outdoors, the AHU uses its warm-up supply temperature.",
        "warm_threshold": "Above 25 °C outdoors, the AHU uses its cooling supply temperature.",
        "warmup_supply": "18 °C supply air is selected in cold weather.",
        "mild_supply": "16 °C supply air is selected in mild weather.",
        "cooling_supply": "14 °C supply air is selected in warm weather.",
        "is_cold": "Compares outdoor temperature with the cold-weather threshold.",
        "is_mild": "Compares outdoor temperature with the warm-weather threshold.",
        "mild_or_cool": "Selects 16 °C or 14 °C supply air for mild or warm weather.",
        "supply_reset": "Selects warm-up supply air in cold weather; otherwise uses the mild/warm result.",
        "fan_gain": "Converts aggregate airflow request into a fan speed command.",
        "fan_limit": "Keeps the supply fan between its minimum 15% speed and full speed.",
        "minimum_outdoor_air": "Commands a fixed 20% outdoor-air share in the conventional example; edit this value to change mixed-air and coil loads.",
        "conventional_return_one": "Fully open return-damper reference.",
        "conventional_return_complement": "Commands the return damper to complement the outdoor damper; both physical commands come from CDL.",
        "coil_zero": "Closes a heating or cooling valve when its mode is not needed.",
        "heating_coil_error": "Finds how much warmer the supply target is than measured mixed air.",
        "heating_coil_feedforward": "Converts a 20 K full-open coil temperature rise into a valve fraction.",
        "heating_coil_feedback": "Trims the heating valve from measured supply-air temperature.",
        "heating_coil_sum": "Combines feedforward and measured-temperature correction.",
        "heating_coil_limit": "Keeps the heating valve within its physical 0–100% travel.",
        "heating_coil_needed": "Enables heating only when mixed air is below the supply target.",
        "heating_coil_enable": "Closes the heating valve when heating is not needed.",
        "cooling_coil_error": "Finds how much warmer mixed air is than the supply target.",
        "cooling_coil_feedforward": "Converts a 20 K full-open coil temperature drop into a valve fraction.",
        "cooling_coil_feedback": "Trims the cooling valve from measured supply-air temperature.",
        "cooling_coil_sum": "Combines feedforward and measured-temperature correction.",
        "cooling_coil_limit": "Keeps the cooling valve within its physical 0–100% travel.",
        "cooling_coil_needed": "Enables cooling only when mixed air is above the supply target.",
        "cooling_coil_enable": "Closes the cooling valve when cooling is not needed.",
    })
    for item in b:
        if item["name"] in {"minimum_outdoor_air", "conventional_return_one", "conventional_return_complement"}:
            for connector in item["inputs"] + item["outputs"]:
                connector["unit"] = "fraction (0–1)"
            for parameter in item["parameters"]:
                parameter["unit"] = "fraction (0–1)"
    for item in b:
        if not (item["name"].startswith("heating_coil_") or item["name"].startswith("cooling_coil_") or item["name"] == "coil_zero"):
            continue
        coil_name = item["name"]
        for connector in item["inputs"] + item["outputs"]:
            if connector["type"] == "Boolean":
                connector["unit"] = "Boolean (0/1)"
            elif coil_name.endswith(("_error", "_needed")) or coil_name.endswith("_feedback") and connector["name"] in ("u_s", "u_m"):
                connector["unit"] = "°C" if connector["name"].startswith("u") else "K"
            elif coil_name.endswith("_feedforward") and connector["name"] == "u":
                connector["unit"] = "K"
            else:
                connector["unit"] = "fraction (0–1)"
        for parameter in item["parameters"]:
            if parameter["name"] == "Ti":
                parameter["unit"] = "s"
            elif parameter["name"] == "k" and coil_name.endswith(("_feedforward", "_feedback")):
                parameter["unit"] = "K⁻¹"
    e = [
        ("outdoor_c", "is_cold.u1"), ("cold_threshold.y", "is_cold.u2"),
        ("outdoor_c", "is_mild.u1"), ("warm_threshold.y", "is_mild.u2"),
        ("mild_supply.y", "mild_or_cool.u1"), ("cooling_supply.y", "mild_or_cool.u2"),
        ("is_mild.y", "mild_or_cool.u3"),
        ("warmup_supply.y", "supply_reset.u1"), ("mild_or_cool.y", "supply_reset.u2"),
        ("is_cold.y", "supply_reset.u3"),
        ("flow_request", "fan_gain.u"), ("fan_gain.y", "fan_limit.u"),
        ("supply_reset.y", "supply_air_temp"), ("fan_limit.y", "fan_speed"),
        ("minimum_outdoor_air.y", "outdoor_air_fraction"),
        ("minimum_outdoor_air.y", "outdoor_air_damper_position"),
        ("conventional_return_one.y", "conventional_return_complement.u1"),
        ("minimum_outdoor_air.y", "conventional_return_complement.u2"),
        ("conventional_return_complement.y", "return_air_damper_position"),
        ("supply_reset.y", "heating_coil_error.u1"),
        ("mixed_air_temp", "heating_coil_error.u2"),
        ("heating_coil_error.y", "heating_coil_feedforward.u"),
        ("supply_reset.y", "heating_coil_feedback.u_s"),
        ("actual_supply_air_temp", "heating_coil_feedback.u_m"),
        ("heating_coil_feedforward.y", "heating_coil_sum.u1"),
        ("heating_coil_feedback.y", "heating_coil_sum.u2"),
        ("heating_coil_sum.y", "heating_coil_limit.u"),
        ("mixed_air_temp", "heating_coil_needed.u1"),
        ("supply_reset.y", "heating_coil_needed.u2"),
        ("heating_coil_limit.y", "heating_coil_enable.u1"),
        ("coil_zero.y", "heating_coil_enable.u2"),
        ("heating_coil_needed.y", "heating_coil_enable.u3"),
        ("heating_coil_enable.y", "heating_coil_valve_position"),
        ("mixed_air_temp", "cooling_coil_error.u1"),
        ("supply_reset.y", "cooling_coil_error.u2"),
        ("cooling_coil_error.y", "cooling_coil_feedforward.u"),
        ("supply_reset.y", "cooling_coil_feedback.u_s"),
        ("actual_supply_air_temp", "cooling_coil_feedback.u_m"),
        ("cooling_coil_feedforward.y", "cooling_coil_sum.u1"),
        ("cooling_coil_feedback.y", "cooling_coil_sum.u2"),
        ("cooling_coil_sum.y", "cooling_coil_limit.u"),
        ("mixed_air_temp", "cooling_coil_needed.u1"),
        ("supply_reset.y", "cooling_coil_needed.u2"),
        ("cooling_coil_limit.y", "cooling_coil_enable.u1"),
        ("coil_zero.y", "cooling_coil_enable.u2"),
        ("cooling_coil_needed.y", "cooling_coil_enable.u3"),
        ("cooling_coil_enable.y", "cooling_coil_valve_position"),
    ]
    result = composite(name, "AHUController", ("outdoor_c", "flow_request", "actual_supply_air_temp", "mixed_air_temp"),
                     ("supply_air_temp", "fan_speed", "outdoor_air_fraction", "outdoor_air_damper_position", "return_air_damper_position", "heating_coil_valve_position", "cooling_coil_valve_position"), b, e,
                     "Supply air temperature reset, fan speed, and coil valve control.")
    result["notes"] = "Serves five VAV zones. Outdoor temperature chooses the supply-air target. Measured mixed and supply temperatures drive editable heating and cooling coil valve loops; total airflow request sets fan speed."
    return result


def plant():
    b = [
        block("one", "Constant", params={"k": 1.0}),
        block("heat_pump_dispatch_fraction", "Constant", params={"k": 0.55}),
        block("heat_pump_dispatch_limit", "Limiter", ("u",), params={"uMin": 0.0, "uMax": 1.0}),
        block("backup_fraction", "Subtract", ("u1", "u2")),
        block("heat_pump_heating", "Multiply", ("u1", "u2")),
        block("heat_pump_cooling", "Multiply", ("u1", "u2")),
        block("boiler", "Multiply", ("u1", "u2")),
        block("chiller", "Multiply", ("u1", "u2")),
        block("pump_zero", "Constant", params={"k": 0.0}),
        block("pump_start_threshold", "Constant", params={"k": 0.5}),
        block("hot_water_pump_needed", "Greater", ("u1", "u2"), boolean_outs=("y",)),
        block("chilled_water_pump_needed", "Greater", ("u1", "u2"), boolean_outs=("y",)),
        block("hot_water_pump_gain", "MultiplyByParameter", ("u",), params={"k": 1.0 / 180.0}),
        block("chilled_water_pump_gain", "MultiplyByParameter", ("u",), params={"k": 1.0 / 180.0}),
        block("hot_water_pump_limit", "Limiter", ("u",), params={"uMin": 0.15, "uMax": 1.0}),
        block("chilled_water_pump_limit", "Limiter", ("u",), params={"uMin": 0.15, "uMax": 1.0}),
        block("hot_water_pump_select", "Switch", ("u1", "u2", "u3"), boolean_ins=("u3",)),
        block("chilled_water_pump_select", "Switch", ("u1", "u2", "u3"), boolean_ins=("u3",)),
        block("hot_water_supply_setpoint", "Constant", params={"k": 45.0}),
        block("chilled_water_supply_setpoint", "Constant", params={"k": 7.0}),
        block("source_zero", "Constant", params={"k": 0.0}),
        block("hot_water_source_error", "Subtract", ("u1", "u2")),
        block("chilled_water_source_error", "Subtract", ("u1", "u2")),
        block("hot_water_source_gain", "MultiplyByParameter", ("u",), params={"k": 6.0}),
        block("chilled_water_source_gain", "MultiplyByParameter", ("u",), params={"k": 6.0}),
        block("hot_water_source_sum", "Add", ("u1", "u2")),
        block("chilled_water_source_sum", "Add", ("u1", "u2")),
        block("hot_water_source_nonnegative", "Max", ("u1", "u2")),
        block("chilled_water_source_nonnegative", "Max", ("u1", "u2")),
        block("heat_pump_heating_enable_gate", "Switch", ("u1", "u2", "u3"), boolean_ins=("u3",)),
        block("heat_pump_cooling_enable_gate", "Switch", ("u1", "u2", "u3"), boolean_ins=("u3",)),
        block("boiler_enable_gate", "Switch", ("u1", "u2", "u3"), boolean_ins=("u3",)),
        block("chiller_enable_gate", "Switch", ("u1", "u2", "u3"), boolean_ins=("u3",)),
    ]
    annotate(b, {
        "one": "Represents 100% of the heating or cooling request.",
        "heat_pump_dispatch_fraction": "Editable heat-pump share of source demand, from zero to one; boiler and chiller receive the remainder.",
        "heat_pump_dispatch_limit": "Keep the editable heat-pump share between zero and one.",
        "backup_fraction": "Calculates the share left for boiler and chiller after heat pump allocation.",
        "heat_pump_heating": "Assigns the selected share of hot-water demand to the heat pump.",
        "heat_pump_cooling": "Assigns the selected share of chilled-water demand to the heat pump.",
        "boiler": "Meets the remaining hot-water demand with the boiler.",
        "chiller": "Meets the remaining chilled-water demand with the chiller.",
        "pump_zero": "Stops both loop pumps when their CDL enable is false.",
        "pump_start_threshold": "Requires more than 0.5 kW of load before enabling a loop.",
        "hot_water_pump_needed": "Enables hot-water circulation from the measured heating request.",
        "chilled_water_pump_needed": "Enables chilled-water circulation from the measured cooling request.",
        "hot_water_pump_gain": "Sizes hot-water pump speed from requested load using the 180 kW design flow.",
        "chilled_water_pump_gain": "Sizes chilled-water pump speed from requested load using the 180 kW design flow.",
        "hot_water_pump_limit": "Keeps an enabled hot-water pump between 15% and full speed.",
        "chilled_water_pump_limit": "Keeps an enabled chilled-water pump between 15% and full speed.",
        "hot_water_pump_select": "Selects the hot-water pump command from its CDL enable.",
        "chilled_water_pump_select": "Selects the chilled-water pump command from its CDL enable.",
        "hot_water_supply_setpoint": "Commands a 45 °C hot-water supply temperature until a reset sequence is added.",
        "chilled_water_supply_setpoint": "Commands a 7 °C chilled-water supply temperature until a reset sequence is added.",
        "source_zero": "Minimum heat or cooling production command is zero kilowatts.",
        "hot_water_source_error": "Hot-water supply target minus measured temperature.",
        "chilled_water_source_error": "Measured chilled-water supply temperature minus its target.",
        "hot_water_source_gain": "Corrects hot-water production by 6 kW per kelvin of supply error; editable for the 5,000 kg volume.",
        "chilled_water_source_gain": "Corrects chilled-water production by 6 kW per kelvin of supply error; editable for the 5,000 kg volume.",
        "hot_water_source_sum": "Adds hot-water temperature correction to the predicted heating load.",
        "chilled_water_source_sum": "Adds chilled-water temperature correction to the predicted cooling load.",
        "hot_water_source_nonnegative": "Prevents a negative hot-water source production command.",
        "chilled_water_source_nonnegative": "Prevents a negative chilled-water source production command.",
        "heat_pump_heating_enable_gate": "Stops heat-pump heating when CDL disables hot-water circulation.",
        "heat_pump_cooling_enable_gate": "Stops heat-pump cooling when CDL disables chilled-water circulation.",
        "boiler_enable_gate": "Stops boiler production when CDL disables hot-water circulation.",
        "chiller_enable_gate": "Stops chiller production when CDL disables chilled-water circulation.",
    })
    pump_units = {
        "one": "fraction (0–1)",
        "heat_pump_dispatch_fraction": "fraction (0–1)",
        "pump_zero": "fraction (0–1)",
        "pump_start_threshold": "kW",
        "hot_water_supply_setpoint": "°C",
        "chilled_water_supply_setpoint": "°C",
        "source_zero": "kW",
    }
    fraction_outputs = {
        "hot_water_pump_gain", "chilled_water_pump_gain",
        "hot_water_pump_limit", "chilled_water_pump_limit",
        "hot_water_pump_select", "chilled_water_pump_select",
    }
    for child in b:
        name = child["name"]
        if name in pump_units:
            child["outputs"][0]["unit"] = pump_units[name]
            child["parameters"][0]["unit"] = pump_units[name]
        if name in fraction_outputs:
            child["outputs"][0]["unit"] = "fraction (0–1)"
        if name == "heat_pump_dispatch_limit":
            for port in child["inputs"] + child["outputs"]:
                port["unit"] = "fraction (0–1)"
        if name.endswith("_pump_gain"):
            child["inputs"][0]["unit"] = "kW"
            child["parameters"][0]["unit"] = "kW⁻¹"
        if name.endswith("_source_error"):
            for port in child["inputs"]:
                port["unit"] = "°C"
            child["outputs"][0]["unit"] = "K"
        if name.endswith("_source_gain"):
            child["inputs"][0]["unit"] = "K"
            child["outputs"][0]["unit"] = "kW"
            child["parameters"][0]["unit"] = "kW/K"
        if name.endswith(("_source_sum", "_source_nonnegative")):
            for port in child["inputs"] + child["outputs"]:
                port["unit"] = "kW"
        if name.endswith("_enable_gate"):
            for port in child["inputs"][:2] + child["outputs"]:
                port["unit"] = "kW"
    e = [
        ("heat_pump_dispatch_fraction.y", "heat_pump_dispatch_limit.u"),
        ("one.y", "backup_fraction.u1"), ("heat_pump_dispatch_limit.y", "backup_fraction.u2"),
        ("hot_water_source_nonnegative.y", "heat_pump_heating.u1"), ("heat_pump_dispatch_limit.y", "heat_pump_heating.u2"),
        ("chilled_water_source_nonnegative.y", "heat_pump_cooling.u1"), ("heat_pump_dispatch_limit.y", "heat_pump_cooling.u2"),
        ("hot_water_source_nonnegative.y", "boiler.u1"), ("backup_fraction.y", "boiler.u2"),
        ("chilled_water_source_nonnegative.y", "chiller.u1"), ("backup_fraction.y", "chiller.u2"),
        ("heat_pump_heating.y", "heat_pump_heating_enable_gate.u1"),
        ("source_zero.y", "heat_pump_heating_enable_gate.u2"),
        ("hot_water_pump_needed.y", "heat_pump_heating_enable_gate.u3"),
        ("heat_pump_heating_enable_gate.y", "heat_pump_heating_kw"),
        ("heat_pump_cooling.y", "heat_pump_cooling_enable_gate.u1"),
        ("source_zero.y", "heat_pump_cooling_enable_gate.u2"),
        ("chilled_water_pump_needed.y", "heat_pump_cooling_enable_gate.u3"),
        ("heat_pump_cooling_enable_gate.y", "heat_pump_cooling_kw"),
        ("boiler.y", "boiler_enable_gate.u1"),
        ("source_zero.y", "boiler_enable_gate.u2"),
        ("hot_water_pump_needed.y", "boiler_enable_gate.u3"),
        ("boiler_enable_gate.y", "boiler_kw"),
        ("chiller.y", "chiller_enable_gate.u1"),
        ("source_zero.y", "chiller_enable_gate.u2"),
        ("chilled_water_pump_needed.y", "chiller_enable_gate.u3"),
        ("chiller_enable_gate.y", "chiller_kw"),
        ("heat_request_kw", "hot_water_pump_needed.u1"),
        ("cool_request_kw", "chilled_water_pump_needed.u1"),
        ("pump_start_threshold.y", "hot_water_pump_needed.u2"),
        ("pump_start_threshold.y", "chilled_water_pump_needed.u2"),
        ("heat_request_kw", "hot_water_pump_gain.u"),
        ("cool_request_kw", "chilled_water_pump_gain.u"),
        ("hot_water_pump_gain.y", "hot_water_pump_limit.u"),
        ("chilled_water_pump_gain.y", "chilled_water_pump_limit.u"),
        ("hot_water_pump_limit.y", "hot_water_pump_select.u1"),
        ("chilled_water_pump_limit.y", "chilled_water_pump_select.u1"),
        ("pump_zero.y", "hot_water_pump_select.u2"),
        ("pump_zero.y", "chilled_water_pump_select.u2"),
        ("hot_water_pump_needed.y", "hot_water_pump_select.u3"),
        ("chilled_water_pump_needed.y", "chilled_water_pump_select.u3"),
        ("hot_water_pump_needed.y", "hot_water_enable"),
        ("chilled_water_pump_needed.y", "chilled_water_enable"),
        ("hot_water_pump_select.y", "hot_water_pump_speed"),
        ("chilled_water_pump_select.y", "chilled_water_pump_speed"),
        ("hot_water_supply_setpoint.y", "hot_water_supply_setpoint_c"),
        ("chilled_water_supply_setpoint.y", "chilled_water_supply_setpoint_c"),
        ("hot_water_supply_setpoint.y", "hot_water_source_error.u1"),
        ("hot_water_supply_temp_c", "hot_water_source_error.u2"),
        ("chilled_water_supply_temp_c", "chilled_water_source_error.u1"),
        ("chilled_water_supply_setpoint.y", "chilled_water_source_error.u2"),
        ("hot_water_source_error.y", "hot_water_source_gain.u"),
        ("chilled_water_source_error.y", "chilled_water_source_gain.u"),
        ("heat_request_kw", "hot_water_source_sum.u1"),
        ("hot_water_source_gain.y", "hot_water_source_sum.u2"),
        ("cool_request_kw", "chilled_water_source_sum.u1"),
        ("chilled_water_source_gain.y", "chilled_water_source_sum.u2"),
        ("hot_water_source_sum.y", "hot_water_source_nonnegative.u1"),
        ("source_zero.y", "hot_water_source_nonnegative.u2"),
        ("chilled_water_source_sum.y", "chilled_water_source_nonnegative.u1"),
        ("source_zero.y", "chilled_water_source_nonnegative.u2"),
    ]
    result = composite("plant", "PlantDispatch", ("heat_request_kw", "cool_request_kw",
                     "hot_water_supply_temp_c", "hot_water_return_temp_c", "chilled_water_supply_temp_c",
                     "chilled_water_return_temp_c", "hot_water_flow_kg_s", "chilled_water_flow_kg_s"),
                     ("heat_pump_heating_kw", "heat_pump_cooling_kw", "boiler_kw", "chiller_kw",
                     "hot_water_enable", "chilled_water_enable", "hot_water_pump_speed", "chilled_water_pump_speed",
                     "hot_water_supply_setpoint_c", "chilled_water_supply_setpoint_c"),
                     b, e, "Dispatch hot and chilled water demand to the heat pump, boiler and chiller.")
    for output in result["outputs"]:
        if output["name"] in ("hot_water_enable", "chilled_water_enable"):
            output["type"] = "Boolean"
    result["notes"] = "Shared four-pipe plant: the heat pump serves a configurable fraction of heating and cooling demand; boiler and chiller cover the remainder."
    return result


def building():
    inputs = ["outdoor_c", "heat_request_kw", "cool_request_kw",
              "ahu_1_flow_request", "ahu_2_flow_request",
              "ahu_1_actual_supply_air_temp", "ahu_2_actual_supply_air_temp",
              "ahu_1_mixed_air_temp", "ahu_2_mixed_air_temp",
              "hot_water_supply_temp_c", "hot_water_return_temp_c", "chilled_water_supply_temp_c",
              "chilled_water_return_temp_c", "hot_water_flow_kg_s", "chilled_water_flow_kg_s"]
    plant_outputs = ("heat_pump_heating_kw", "heat_pump_cooling_kw", "boiler_kw", "chiller_kw",
                     "hot_water_enable", "chilled_water_enable", "hot_water_pump_speed", "chilled_water_pump_speed",
                     "hot_water_supply_setpoint_c", "chilled_water_supply_setpoint_c")
    outputs = [f"plant_{x}" for x in plant_outputs]
    blocks = [plant(), ahu("ahu_1"), ahu("ahu_2")]
    edges = [
        ("outdoor_c", "ahu_1.outdoor_c"), ("outdoor_c", "ahu_2.outdoor_c"),
        ("ahu_1_flow_request", "ahu_1.flow_request"), ("ahu_2_flow_request", "ahu_2.flow_request"),
        ("ahu_1_actual_supply_air_temp", "ahu_1.actual_supply_air_temp"),
        ("ahu_2_actual_supply_air_temp", "ahu_2.actual_supply_air_temp"),
        ("ahu_1_mixed_air_temp", "ahu_1.mixed_air_temp"),
        ("ahu_2_mixed_air_temp", "ahu_2.mixed_air_temp"),
        ("heat_request_kw", "plant.heat_request_kw"), ("cool_request_kw", "plant.cool_request_kw"),
        ("hot_water_supply_temp_c", "plant.hot_water_supply_temp_c"),
        ("hot_water_return_temp_c", "plant.hot_water_return_temp_c"),
        ("chilled_water_supply_temp_c", "plant.chilled_water_supply_temp_c"),
        ("chilled_water_return_temp_c", "plant.chilled_water_return_temp_c"),
        ("hot_water_flow_kg_s", "plant.hot_water_flow_kg_s"),
        ("chilled_water_flow_kg_s", "plant.chilled_water_flow_kg_s"),
    ]
    for ahu_name in ("ahu_1", "ahu_2"):
        for output in ("supply_air_temp", "fan_speed", "outdoor_air_fraction", "outdoor_air_damper_position", "return_air_damper_position", "heating_coil_valve_position", "cooling_coil_valve_position"):
            outputs.append(f"{ahu_name}_{output}")
            edges.append((f"{ahu_name}.{output}", f"{ahu_name}_{output}"))
    for output in plant_outputs:
        edges.append((f"plant.{output}", f"plant_{output}"))
    for n in range(10):
        name = f"zone_{n + 1:02d}"
        ahu_name = f"ahu_{n // 5 + 1}"
        blocks.append(vav(name, 23.5 if n % 5 in (0, 4) else 24.0))
        inputs.extend((f"{name}_room_temp", f"{name}_supply_pressure"))
        edges.extend(((f"{name}_room_temp", f"{name}.room_temp"),
                      (f"{ahu_name}_actual_supply_air_temp", f"{name}.supply_air_temp"),
                      (f"{name}_supply_pressure", f"{name}.supply_pressure")))
        for output in ("damper_position", "reheat_valve_position", "airflow", "airflow_request", "cooling_demand", "heating_demand"):
            outputs.append(f"{name}_{output}")
            edges.append((f"{name}.{output}", f"{name}_{output}"))
    result = composite("FourPipeBuildingControls", "BuildingController", inputs, outputs, blocks, edges,
                       "Nested VAV, AHU and plant control sequences for the four-pipe building.")
    for output in result["outputs"]:
        if output["name"] in ("plant_hot_water_enable", "plant_chilled_water_enable"):
            output["type"] = "Boolean"
    result["notes"] = "Two AHUs each serve five VAV zones. VAVs and AHU coil loops receive measured supply-air temperature from the physical model. AHU blocks command coil valves and the supply target; the shared plant serves coil and reheat demand."
    entities = [
        {"id": "building_01", "class": "rec:Building", "label": "Example four-pipe building"},
    ]
    relationships = []
    for ahu_index in (1, 2):
        entities.append({"id": f"ahu_{ahu_index}_equipment", "class": "brick:AHU",
                         "label": f"AHU {ahu_index}", "controller": f"ahu_{ahu_index}"})
    for zone_index in range(1, 11):
        zone = f"zone_{zone_index:02d}"
        ahu_index = (zone_index - 1) // 5 + 1
        room, hvac_zone, vav_id, sensor = (f"space_{zone_index:02d}", f"hvac_{zone}",
                                          f"vav_{zone_index:02d}_equipment", f"{zone}_room_temp")
        entities.extend([
            {"id": room, "class": "rec:Room", "label": f"Space {zone_index:02d}"},
            {"id": hvac_zone, "class": "brick:HVAC_Zone", "label": f"HVAC Zone {zone_index:02d}",
             "controller": zone},
            {"id": vav_id, "class": "brick:VAV", "label": f"VAV {zone_index:02d}",
             "controller": zone},
            {"id": sensor, "class": "brick:Zone_Air_Temperature_Sensor",
             "label": f"Zone {zone_index:02d} air temperature", "connector": sensor},
        ])
        relationships.extend([
            {"subject": "building_01", "predicate": "brick:hasPart", "object": room},
            {"subject": hvac_zone, "predicate": "brick:hasPart", "object": room},
            {"subject": f"ahu_{ahu_index}_equipment", "predicate": "brick:feeds", "object": vav_id},
            {"subject": vav_id, "predicate": "brick:feeds", "object": hvac_zone},
            {"subject": hvac_zone, "predicate": "brick:hasPoint", "object": sensor},
        ])
    result["brick_model"] = {"entities": entities, "relationships": relationships}
    annotate_points(result)
    return result
