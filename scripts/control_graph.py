"""Factory for the editable building control graph."""
import json


# Brick describes the role of a point; these labels are for people and the
# existing connector names remain stable identifiers in saved control graphs.
POINTS = {
    "BuildingController": {
        "outdoor_c": ("Outside Air Temperature Sensor", "Outside_Air_Temperature_Sensor", "°C"),
        "heat_request_kw": ("Heating Load Request", "Point", "kW"),
        "cool_request_kw": ("Cooling Load Request", "Point", "kW"),
        "heat_pump_fraction": ("Heat Pump Dispatch Fraction", "Parameter", "fraction (0–1)"),
        "ahu_1_flow_request": ("AHU 1 Supply Air Flow Request", "Point", "normalized airflow"),
        "ahu_2_flow_request": ("AHU 2 Supply Air Flow Request", "Point", "normalized airflow"),
        "ahu_1_actual_supply_air_temp": ("AHU 1 Supply Air Temperature Sensor", "Supply_Air_Temperature_Sensor", "°C"),
        "ahu_2_actual_supply_air_temp": ("AHU 2 Supply Air Temperature Sensor", "Supply_Air_Temperature_Sensor", "°C"),
        "ahu_1_supply_air_temp": ("AHU 1 Supply Air Temperature Setpoint", "Supply_Air_Temperature_Setpoint", "°C"),
        "ahu_2_supply_air_temp": ("AHU 2 Supply Air Temperature Setpoint", "Supply_Air_Temperature_Setpoint", "°C"),
        "ahu_1_fan_speed": ("AHU 1 Supply Fan Speed Command", "Fan_Speed_Command", "fraction (0–1)"),
        "ahu_2_fan_speed": ("AHU 2 Supply Fan Speed Command", "Fan_Speed_Command", "fraction (0–1)"),
    },
    "VAVBoxController": {
        "room_temp": ("Zone Air Temperature Sensor", "Zone_Air_Temperature_Sensor", "°C"),
        "supply_air_temp": ("Supply Air Temperature Sensor", "Supply_Air_Temperature_Sensor", "°C"),
        "supply_pressure": ("Supply Air Static Pressure Input", "Point", "normalized pressure"),
        "damper_position": ("Damper Position Command", "Damper_Position_Command", "fraction (0–1)"),
        "reheat_valve_position": ("Reheat Valve Position Command", "Valve_Position_Command", "fraction (0–1)"),
        "airflow": ("Supply Air Flow Estimate", "Point", "normalized airflow"),
        "cooling_demand": ("Cooling Demand Estimate", "Point", "fraction (0–1)"),
        "heating_demand": ("Heating Demand Estimate", "Point", "fraction (0–1)"),
    },
    "AHUController": {
        "outdoor_c": ("Outside Air Temperature Sensor", "Outside_Air_Temperature_Sensor", "°C"),
        "flow_request": ("Supply Air Flow Request", "Point", "normalized airflow"),
        "supply_air_temp": ("Supply Air Temperature Setpoint", "Supply_Air_Temperature_Setpoint", "°C"),
        "fan_speed": ("Supply Fan Speed Command", "Fan_Speed_Command", "fraction (0–1)"),
    },
    "PlantDispatch": {
        "heat_request_kw": ("Heating Load Request", "Point", "kW"),
        "cool_request_kw": ("Cooling Load Request", "Point", "kW"),
        "heat_pump_fraction": ("Heat Pump Dispatch Fraction", "Parameter", "fraction (0–1)"),
        "heat_pump_heating_kw": ("Heat Pump Heating Command", "Heating_Command", "kW"),
        "heat_pump_cooling_kw": ("Heat Pump Cooling Command", "Cooling_Command", "kW"),
        "boiler_kw": ("Boiler Heating Command", "Heating_Command", "kW"),
        "chiller_kw": ("Chiller Cooling Command", "Cooling_Command", "kW"),
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
        ("cool_error.y", "cool_demand_gain.u"), ("cool_demand_gain.y", "cool_demand_limit.u"),
        ("heat_error.y", "heat_demand_gain.u"), ("heat_demand_gain.y", "heat_demand_limit.u"),
        ("damper_select.y", "damper_position"), ("reheat_select.y", "reheat_valve_position"),
        ("airflow_sum.y", "airflow"), ("cool_demand_limit.y", "cooling_demand"),
        ("heat_demand_limit.y", "heating_demand"),
    ]
    result = composite(name, "VAVBoxController", ("room_temp", "supply_air_temp", "supply_pressure"),
                     ("damper_position", "reheat_valve_position", "airflow", "cooling_demand", "heating_demand"),
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
    })
    e = [
        ("outdoor_c", "is_cold.u1"), ("cold_threshold.y", "is_cold.u2"),
        ("outdoor_c", "is_mild.u1"), ("warm_threshold.y", "is_mild.u2"),
        ("mild_supply.y", "mild_or_cool.u1"), ("cooling_supply.y", "mild_or_cool.u2"),
        ("is_mild.y", "mild_or_cool.u3"),
        ("warmup_supply.y", "supply_reset.u1"), ("mild_or_cool.y", "supply_reset.u2"),
        ("is_cold.y", "supply_reset.u3"),
        ("flow_request", "fan_gain.u"), ("fan_gain.y", "fan_limit.u"),
        ("supply_reset.y", "supply_air_temp"), ("fan_limit.y", "fan_speed"),
    ]
    result = composite(name, "AHUController", ("outdoor_c", "flow_request"),
                     ("supply_air_temp", "fan_speed"), b, e,
                     "Supply air temperature reset and fan speed control.")
    result["notes"] = "Serves five VAV zones. Outdoor temperature chooses the supply air target; total airflow request sets the supply fan speed."
    return result


def plant():
    b = [
        block("one", "Constant", params={"k": 1.0}),
        block("backup_fraction", "Subtract", ("u1", "u2")),
        block("heat_pump_heating", "Multiply", ("u1", "u2")),
        block("heat_pump_cooling", "Multiply", ("u1", "u2")),
        block("boiler", "Multiply", ("u1", "u2")),
        block("chiller", "Multiply", ("u1", "u2")),
    ]
    annotate(b, {
        "one": "Represents 100% of the heating or cooling request.",
        "backup_fraction": "Calculates the share left for boiler and chiller after heat pump allocation.",
        "heat_pump_heating": "Assigns the selected share of hot-water demand to the heat pump.",
        "heat_pump_cooling": "Assigns the selected share of chilled-water demand to the heat pump.",
        "boiler": "Meets the remaining hot-water demand with the boiler.",
        "chiller": "Meets the remaining chilled-water demand with the chiller.",
    })
    e = [
        ("one.y", "backup_fraction.u1"), ("heat_pump_fraction", "backup_fraction.u2"),
        ("heat_request_kw", "heat_pump_heating.u1"), ("heat_pump_fraction", "heat_pump_heating.u2"),
        ("cool_request_kw", "heat_pump_cooling.u1"), ("heat_pump_fraction", "heat_pump_cooling.u2"),
        ("heat_request_kw", "boiler.u1"), ("backup_fraction.y", "boiler.u2"),
        ("cool_request_kw", "chiller.u1"), ("backup_fraction.y", "chiller.u2"),
        ("heat_pump_heating.y", "heat_pump_heating_kw"),
        ("heat_pump_cooling.y", "heat_pump_cooling_kw"),
        ("boiler.y", "boiler_kw"), ("chiller.y", "chiller_kw"),
    ]
    result = composite("plant", "PlantDispatch", ("heat_request_kw", "cool_request_kw", "heat_pump_fraction"),
                     ("heat_pump_heating_kw", "heat_pump_cooling_kw", "boiler_kw", "chiller_kw"),
                     b, e, "Dispatch hot and chilled water demand to the heat pump, boiler and chiller.")
    result["notes"] = "Shared four-pipe plant: the heat pump serves a configurable fraction of heating and cooling demand; boiler and chiller cover the remainder."
    return result


def building():
    inputs = ["outdoor_c", "heat_request_kw", "cool_request_kw", "heat_pump_fraction",
              "ahu_1_flow_request", "ahu_2_flow_request",
              "ahu_1_actual_supply_air_temp", "ahu_2_actual_supply_air_temp"]
    outputs = [f"plant_{x}" for x in ("heat_pump_heating_kw", "heat_pump_cooling_kw", "boiler_kw", "chiller_kw")]
    blocks = [plant(), ahu("ahu_1"), ahu("ahu_2")]
    edges = [
        ("outdoor_c", "ahu_1.outdoor_c"), ("outdoor_c", "ahu_2.outdoor_c"),
        ("ahu_1_flow_request", "ahu_1.flow_request"), ("ahu_2_flow_request", "ahu_2.flow_request"),
        ("heat_request_kw", "plant.heat_request_kw"), ("cool_request_kw", "plant.cool_request_kw"),
        ("heat_pump_fraction", "plant.heat_pump_fraction"),
    ]
    for ahu_name in ("ahu_1", "ahu_2"):
        for output in ("supply_air_temp", "fan_speed"):
            outputs.append(f"{ahu_name}_{output}")
            edges.append((f"{ahu_name}.{output}", f"{ahu_name}_{output}"))
    for output in ("heat_pump_heating_kw", "heat_pump_cooling_kw", "boiler_kw", "chiller_kw"):
        edges.append((f"plant.{output}", f"plant_{output}"))
    for n in range(10):
        name = f"zone_{n + 1:02d}"
        ahu_name = f"ahu_{n // 5 + 1}"
        blocks.append(vav(name, 23.5 if n % 5 in (0, 4) else 24.0))
        inputs.extend((f"{name}_room_temp", f"{name}_supply_pressure"))
        edges.extend(((f"{name}_room_temp", f"{name}.room_temp"),
                      (f"{ahu_name}_actual_supply_air_temp", f"{name}.supply_air_temp"),
                      (f"{name}_supply_pressure", f"{name}.supply_pressure")))
        for output in ("damper_position", "reheat_valve_position", "airflow", "cooling_demand", "heating_demand"):
            outputs.append(f"{name}_{output}")
            edges.append((f"{name}.{output}", f"{name}_{output}"))
    result = composite("FourPipeBuildingControls", "BuildingController", inputs, outputs, blocks, edges,
                       "Nested VAV, AHU and plant control sequences for the four-pipe building.")
    result["notes"] = "Two AHUs each serve five VAV zones. VAVs receive measured supply air temperature from the physical model, while AHU blocks command its target. The shared plant serves coil and reheat demand."
    annotate_points(result)
    return result
