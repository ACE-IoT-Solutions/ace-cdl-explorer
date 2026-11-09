"""Editable G36 reheat-terminal low discharge temperature alarms."""


def add_low_discharge_alarms(graph, port, constant, binary):
    def wire(source, target):
        return {"from": source, "to": target}

    graph["inputs"].append(port(
        "hot_water_plant_proven", "Boolean", "unitless", "Hot Water Plant Heating Proven",
        "Status", "Physical heating production during the previous simulation step; false when only cooling runs",
    ))
    for index in range(1, 11):
        name = f"zone_{index:02}"
        zone = next(block for block in graph["blocks"] if block["name"] == name)
        zone["inputs"].append(port("hot_water_plant_proven", "Boolean", "unitless",
                                   "Hot Water Plant Heating Proven", "Status"))
        graph["connections"].append(wire("hot_water_plant_proven", f"{name}.hot_water_plant_proven"))
        blocks, edges = zone["blocks"], zone["connections"]
        blocks.extend([
            constant("low_discharge_importance", 1.0, "fraction (0–1)",
                     "Set to zero to suppress this zone's hot-water reset alarms."),
            constant("low_discharge_zero_importance", 0.0, "fraction (0–1)",
                     "Alarm suppression threshold."),
            binary("low_discharge_important", "Greater", "Real", "Boolean",
                   "Hot-water reset importance must be positive to allow alarms."),
            binary("low_discharge_fan_and_plant", "And", "Boolean", "Boolean",
                   "The AHU supply fan and hot-water plant must both be physically proven."),
            binary("low_discharge_occupied_and_plant", "And", "Boolean", "Boolean",
                   "The zone is occupied and the AHU fan and hot-water plant are proven."),
            binary("low_discharge_eligible", "And", "Boolean", "Boolean",
                   "Suppress temperature alarms when hot-water reset importance is zero."),
            {"name": "low_discharge_error", "block_type": "Subtract", "category": "elementary",
             "inputs": [port("u1", unit="°C"), port("u2", unit="°C")],
             "outputs": [port("y", unit="K")],
             "notes": "Discharge setpoint minus measured discharge temperature."},
            constant("low_discharge_level_three_threshold", 8.0, "K",
                     "Level 3 when measured discharge is more than 8 K below setpoint."),
            constant("low_discharge_level_two_threshold", 17.0, "K",
                     "Level 2 when measured discharge is more than 17 K below setpoint."),
            {"name": "low_discharge_below_eight", "block_type": "Greater", "category": "elementary",
             "inputs": [port("u1", unit="K"), port("u2", unit="K")],
             "outputs": [port("y", "Boolean")],
             "notes": "Measured discharge more than 8 K below its CDL setpoint."},
            {"name": "low_discharge_below_seventeen", "block_type": "Greater", "category": "elementary",
             "inputs": [port("u1", unit="K"), port("u2", unit="K")],
             "outputs": [port("y", "Boolean")],
             "notes": "Measured discharge more than 17 K below its CDL setpoint."},
            binary("low_discharge_three_condition", "And", "Boolean", "Boolean",
                   "Publish the timed Level 3 error only while occupied, important, and the hot-water plant is proven."),
            binary("low_discharge_two_condition", "And", "Boolean", "Boolean",
                   "Publish the timed Level 2 error only while occupied, important, and the hot-water plant is proven."),
            *[{
                "name": f"low_discharge_{level}_delay", "block_type": "TrueDelay", "category": "elementary",
                "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
                "parameters": [{"name": "delayTime", "value": 600.0, "unit": "s"},
                               {"name": "delayOnInit", "type": "Boolean", "value": True}],
                "notes": "Time the discharge temperature error independently of occupancy and plant proof, as in OBC Alarms.",
            } for level in ("three", "two")],
            *[{
                "name": f"low_discharge_level_{level}",
                "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
                "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "level")],
                "parameters": [{"name": "k", "type": "Integer", "value": value}],
                "notes": f"Level {value} low discharge temperature alarm." if value else "No temperature alarm.",
            } for level, value in (("zero", 0), ("three", 3), ("two", 2))],
            *[{
                "name": f"low_discharge_{name}", "block_type": "Buildings.Controls.OBC.CDL.Integers.Switch",
                "category": "elementary",
                "inputs": [port("u1", "Integer", "level"), port("u2", "Boolean"),
                           port("u3", "Integer", "level")],
                "outputs": [port("y", "Integer", "level")], "notes": note,
            } for name, note in (("moderate", "Publish Level 3 after its ten-minute delay."),
                                 ("priority", "Severe Level 2 takes precedence over Level 3."))],
        ])
        edges.extend(wire(source, target) for source, target in [
            ("low_discharge_importance.y", "low_discharge_important.u1"),
            ("low_discharge_zero_importance.y", "low_discharge_important.u2"),
            ("supply_fan_proven", "low_discharge_fan_and_plant.u1"),
            ("hot_water_plant_proven", "low_discharge_fan_and_plant.u2"),
            ("local_occupied.y", "low_discharge_occupied_and_plant.u1"),
            ("low_discharge_fan_and_plant.y", "low_discharge_occupied_and_plant.u2"),
            ("low_discharge_occupied_and_plant.y", "low_discharge_eligible.u1"),
            ("low_discharge_important.y", "low_discharge_eligible.u2"),
            ("discharge_target.y", "low_discharge_error.u1"),
            ("discharge_temp", "low_discharge_error.u2"),
            ("low_discharge_error.y", "low_discharge_below_eight.u1"),
            ("low_discharge_level_three_threshold.y", "low_discharge_below_eight.u2"),
            ("low_discharge_error.y", "low_discharge_below_seventeen.u1"),
            ("low_discharge_level_two_threshold.y", "low_discharge_below_seventeen.u2"),
            ("low_discharge_below_eight.y", "low_discharge_three_delay.u"),
            ("low_discharge_below_seventeen.y", "low_discharge_two_delay.u"),
            ("low_discharge_eligible.y", "low_discharge_three_condition.u1"),
            ("low_discharge_three_delay.y", "low_discharge_three_condition.u2"),
            ("low_discharge_eligible.y", "low_discharge_two_condition.u1"),
            ("low_discharge_two_delay.y", "low_discharge_two_condition.u2"),
            ("low_discharge_level_three.y", "low_discharge_moderate.u1"),
            ("low_discharge_three_condition.y", "low_discharge_moderate.u2"),
            ("low_discharge_level_zero.y", "low_discharge_moderate.u3"),
            ("low_discharge_level_two.y", "low_discharge_priority.u1"),
            ("low_discharge_two_condition.y", "low_discharge_priority.u2"),
            ("low_discharge_moderate.y", "low_discharge_priority.u3"),
            ("low_discharge_priority.y", "low_discharge_temperature_alarm_level"),
        ])
        zone["outputs"].append(port("low_discharge_temperature_alarm_level", "Integer", "level",
                                    "Low Discharge Air Temperature Alarm", "Alarm"))
        point = f"{name}_low_discharge_temperature_alarm_level"
        graph["outputs"].append(port(point, "Integer", "level",
                                     f"Zone {index} Low Discharge Air Temperature Alarm", "Alarm"))
        graph["connections"].append(wire(f"{name}.low_discharge_temperature_alarm_level", point))
        zone["notes"] += " Low-discharge error timers run on measured temperature; occupied mode, physical fan and heating-production proof, and editable reset importance gate the published Level 3/2 alarms."
    graph["notes"] += " VAV low-discharge alarms are calculated in editable CDL from measured temperatures and physical hot-water production proof."
