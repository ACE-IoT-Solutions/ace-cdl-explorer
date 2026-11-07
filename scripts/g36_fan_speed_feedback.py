"""Expose physical supply-fan speed and diagnose drive tracking in editable CDL."""


def add_fan_speed_feedback(graph, port, constant, binary):
    def wire(source, target):
        return {"from": source, "to": target}

    for index in (1, 2):
        name = f"ahu_{index}"
        ahu = next(block for block in graph["blocks"] if block["name"] == name)
        measured = f"{name}_actual_fan_speed"
        graph["inputs"].append(port(measured, unit="fraction (0–1)",
                                    label=f"AHU {index} Supply Fan Speed Feedback",
                                    brick="Motor_Speed_Sensor", description="Measured normalized drive speed from the previous physical step"))
        ahu["inputs"].append(port("actual_fan_speed", unit="fraction (0–1)",
                                  label="Supply Fan Speed Feedback", brick="Motor_Speed_Sensor"))
        graph["connections"].append(wire(measured, f"{name}.actual_fan_speed"))
        ahu["blocks"].extend([
            {"name": "fan_speed_tracking_error", "block_type": "Subtract", "category": "elementary",
             "inputs": [port("u1", unit="fraction (0–1)"), port("u2", unit="fraction (0–1)")],
             "outputs": [port("y", unit="fraction (0–1)")],
             "notes": "Final CDL fan speed command minus measured drive speed."},
            constant("fan_speed_tracking_tolerance", 0.15, "fraction (0–1)",
                     "Allow 15 percentage points of drive speed tracking error."),
            binary("fan_speed_tracking_low", "Greater", note="Physical drive speed trails the CDL command."),
            constant("fan_speed_tracking_min_command", 0.4, "fraction (0–1)",
                     "Diagnose speed tracking only when the fan is commanded above 40%."),
            binary("fan_speed_tracking_enabled", "Greater", note="Fan speed command is large enough to diagnose tracking."),
            binary("fan_speed_tracking_active", "And", "Boolean", "Boolean",
                   "Fan is proven on and the command exceeds the diagnostic minimum."),
            binary("fan_speed_tracking_condition", "And", "Boolean", "Boolean",
                   "Measured speed remains below the command while the fan runs."),
            {"name": "fan_speed_tracking_delay", "block_type": "TrueDelay", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "delayTime", "value": 300.0, "unit": "s"},
                            {"name": "delayOnInit", "type": "Boolean", "value": True}],
             "notes": "Require five continuous minutes of physical speed deficit."},
            {"name": "fan_speed_tracking_clear", "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
             "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "level")],
             "parameters": [{"name": "k", "type": "Integer", "value": 0}],
             "notes": "No supply-fan speed tracking alarm."},
            {"name": "fan_speed_tracking_level_three", "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
             "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "level")],
             "parameters": [{"name": "k", "type": "Integer", "value": 3}],
             "notes": "Teaching Level 3 drive speed tracking alarm."},
            {"name": "fan_speed_tracking_select", "block_type": "Buildings.Controls.OBC.CDL.Integers.Switch",
             "category": "elementary",
             "inputs": [port("u1", "Integer", "level"), port("u2", "Boolean"),
                        port("u3", "Integer", "level")],
             "outputs": [port("y", "Integer", "level")],
             "notes": "Publish the speed tracking alarm from the timed CDL condition."},
        ])
        for block in ahu["blocks"]:
            if block["name"] in {"fan_speed_tracking_low", "fan_speed_tracking_enabled"}:
                for connector in block["inputs"]:
                    connector["unit"] = "fraction (0–1)"
        ahu["connections"].extend(wire(source, target) for source, target in [
            ("freeze_stage_three_fan.y", "fan_speed_tracking_error.u1"),
            ("actual_fan_speed", "fan_speed_tracking_error.u2"),
            ("fan_speed_tracking_error.y", "fan_speed_tracking_low.u1"),
            ("fan_speed_tracking_tolerance.y", "fan_speed_tracking_low.u2"),
            ("freeze_stage_three_fan.y", "fan_speed_tracking_enabled.u1"),
            ("fan_speed_tracking_min_command.y", "fan_speed_tracking_enabled.u2"),
            ("fan_speed_tracking_enabled.y", "fan_speed_tracking_active.u1"),
            ("supply_fan_proven", "fan_speed_tracking_active.u2"),
            ("fan_speed_tracking_active.y", "fan_speed_tracking_condition.u1"),
            ("fan_speed_tracking_low.y", "fan_speed_tracking_condition.u2"),
            ("fan_speed_tracking_condition.y", "fan_speed_tracking_delay.u"),
            ("fan_speed_tracking_level_three.y", "fan_speed_tracking_select.u1"),
            ("fan_speed_tracking_delay.y", "fan_speed_tracking_select.u2"),
            ("fan_speed_tracking_clear.y", "fan_speed_tracking_select.u3"),
            ("fan_speed_tracking_select.y", "fan_speed_tracking_alarm_level"),
        ])
        ahu["outputs"].append(port("fan_speed_tracking_alarm_level", "Integer", "level",
                                   "Supply Fan Speed Tracking Alarm", "Alarm"))
        output = f"{name}_fan_speed_tracking_alarm_level"
        graph["outputs"].append(port(output, "Integer", "level",
                                     f"AHU {index} Supply Fan Speed Tracking Alarm", "Alarm"))
        graph["connections"].append(wire(f"{name}.fan_speed_tracking_alarm_level", output))
        ahu["notes"] += " Measured fan speed feeds an editable five-minute drive speed tracking diagnostic."
    graph["notes"] += " Supply fan drive speed feedback is physical; its tracking alarm is CDL."
