"""Editable AHU fan command/status discrepancy alarms."""


def add_supply_fan_alarms(graph, port, constant, binary):
    def wire(source, target):
        return {"from": source, "to": target}

    def invert(name, note):
        return {"name": name, "block_type": "Not", "category": "elementary",
                "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
                "notes": note}

    def hold(name, seconds, note):
        return {"name": name, "block_type": "TrueDelay", "category": "elementary",
                "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
                "parameters": [{"name": "delayTime", "value": seconds, "unit": "s"},
                               {"name": "delayOnInit", "type": "Boolean", "value": True}],
                "notes": note}

    def level(name, value, note):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
                "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "level")],
                "parameters": [{"name": "k", "type": "Integer", "value": value}], "notes": note}

    def choose(name, note):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Integers.Switch",
                "category": "elementary", "inputs": [port("u1", "Integer", "level"),
                                                       port("u2", "Boolean"),
                                                       port("u3", "Integer", "level")],
                "outputs": [port("y", "Integer", "level")], "notes": note}

    for index in (1, 2):
        name = f"ahu_{index}"
        ahu = next(block for block in graph["blocks"] if block["name"] == name)
        root_status = next(point for point in graph["inputs"]
                           if point["name"] == f"{name}_supply_fan_proven")
        root_status["brick_class"] = "Fan_Status"
        root_status["description"] = "Measured fan motor running status from the previous physical step, independent of terminal airflow"
        ahu_status = next(point for point in ahu["inputs"] if point["name"] == "supply_fan_proven")
        ahu_status["brick_class"] = "Fan_Status"
        ahu["blocks"].extend([
            constant("fan_alarm_command_threshold", 0.01, "fraction (0–1)",
                     "Treat a speed command above 1% as a fan-on command."),
            binary("fan_alarm_commanded_on", "Greater", note="Classify the final supply fan command."),
            invert("fan_alarm_status_off", "True when physical supply fan status is off."),
            invert("fan_alarm_commanded_off", "True when CDL commands the supply fan off."),
            binary("fan_alarm_failed_start", "And", "Boolean", "Boolean",
                   "Commanded on with physical status off."),
            binary("fan_alarm_failed_stop", "And", "Boolean", "Boolean",
                   "Commanded off with physical status on."),
            hold("fan_alarm_start_delay", 15.0,
                 "Require a 15-second on-command/status discrepancy before Level 2."),
            hold("fan_alarm_stop_delay", 60.0,
                 "Require a 60-second off-command/status discrepancy before Level 4."),
            level("fan_alarm_clear", 0, "No fan command/status discrepancy."),
            level("fan_alarm_level_two", 2, "Level 2 fan commanded on but not running."),
            level("fan_alarm_level_four", 4, "Level 4 fan running while commanded off."),
            choose("fan_alarm_stop_select", "Expose the Level 4 failed-stop alarm."),
            choose("fan_alarm_priority", "Level 2 failed-start alarm takes priority."),
        ])
        ahu["connections"].extend(wire(source, target) for source, target in [
            ("freeze_stage_three_fan.y", "fan_alarm_commanded_on.u1"),
            ("fan_alarm_command_threshold.y", "fan_alarm_commanded_on.u2"),
            ("supply_fan_proven", "fan_alarm_status_off.u"),
            ("fan_alarm_commanded_on.y", "fan_alarm_commanded_off.u"),
            ("fan_alarm_commanded_on.y", "fan_alarm_failed_start.u1"),
            ("fan_alarm_status_off.y", "fan_alarm_failed_start.u2"),
            ("fan_alarm_commanded_off.y", "fan_alarm_failed_stop.u1"),
            ("supply_fan_proven", "fan_alarm_failed_stop.u2"),
            ("fan_alarm_failed_start.y", "fan_alarm_start_delay.u"),
            ("fan_alarm_failed_stop.y", "fan_alarm_stop_delay.u"),
            ("fan_alarm_level_four.y", "fan_alarm_stop_select.u1"),
            ("fan_alarm_stop_delay.y", "fan_alarm_stop_select.u2"),
            ("fan_alarm_clear.y", "fan_alarm_stop_select.u3"),
            ("fan_alarm_level_two.y", "fan_alarm_priority.u1"),
            ("fan_alarm_start_delay.y", "fan_alarm_priority.u2"),
            ("fan_alarm_stop_select.y", "fan_alarm_priority.u3"),
            ("fan_alarm_priority.y", "supply_fan_alarm_level"),
        ])
        for connector in next(block for block in ahu["blocks"]
                              if block["name"] == "fan_alarm_commanded_on")["inputs"]:
            connector["unit"] = "fraction (0–1)"
        ahu["outputs"].append(port("supply_fan_alarm_level", "Integer", "level",
                                   "Supply Fan Status Alarm", "Alarm"))
        root_output = f"{name}_supply_fan_alarm_level"
        graph["outputs"].append(port(root_output, "Integer", "level",
                                     f"AHU {index} Supply Fan Status Alarm", "Alarm"))
        graph["connections"].append(wire(f"{name}.supply_fan_alarm_level", root_output))
        ahu["notes"] += " Supply fan command/status alarms use editable 15-second start and 60-second stop delays."
    graph["notes"] += " Physical fan status remains a raw input; editable CDL decides fan discrepancy alarms."
