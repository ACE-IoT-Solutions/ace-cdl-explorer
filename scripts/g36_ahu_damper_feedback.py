"""Physical AHU damper feedback and editable command/position diagnostics."""


def add_ahu_damper_feedback(graph, port, constant, binary):
    def wire(source, target):
        return {"from": source, "to": target}

    for index in (1, 2):
        name = f"ahu_{index}"
        ahu = next(block for block in graph["blocks"] if block["name"] == name)
        for damper, label in (("outdoor", "Outdoor Air"), ("return", "Return Air")):
            prefix = f"{damper}_damper_tracking"
            sensor = f"{name}_actual_{damper}_damper_position"
            graph["inputs"].append(port(sensor, unit="fraction (0–1)",
                                        label=f"AHU {index} {label} Damper Position Sensor",
                                        brick="Damper_Position_Sensor",
                                        description="Physical damper opening from the previous step, after any stuck-position fault"))
            ahu["inputs"].append(port(f"actual_{damper}_damper_position", unit="fraction (0–1)",
                                      label=f"{label} Damper Position Sensor", brick="Damper_Position_Sensor"))
            graph["connections"].append(wire(sensor, f"{name}.actual_{damper}_damper_position"))
            ahu["blocks"].extend([
                {"name": f"{prefix}_high_error", "block_type": "Subtract", "category": "elementary",
                 "inputs": [port("u1", unit="fraction (0–1)"), port("u2", unit="fraction (0–1)")],
                 "outputs": [port("y", unit="fraction (0–1)")],
                 "notes": "Commanded opening minus measured opening."},
                {"name": f"{prefix}_low_error", "block_type": "Subtract", "category": "elementary",
                 "inputs": [port("u1", unit="fraction (0–1)"), port("u2", unit="fraction (0–1)")],
                 "outputs": [port("y", unit="fraction (0–1)")],
                 "notes": "Measured opening minus commanded opening."},
                binary(f"{prefix}_absolute_error", "Max", "Real", "Real",
                       "Absolute command/position difference."),
                constant(f"{prefix}_tolerance", 0.25, "fraction (0–1)",
                         "Teaching alarm threshold: 25 percentage points of position error."),
                binary(f"{prefix}_error_high", "Greater", note="Position error exceeds the editable threshold."),
                binary(f"{prefix}_condition", "And", "Boolean", "Boolean",
                       "AHU fan is proven on while damper position fails to follow command."),
                {"name": f"{prefix}_delay", "block_type": "TrueDelay", "category": "elementary",
                 "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
                 "parameters": [{"name": "delayTime", "value": 600.0, "unit": "s"},
                                {"name": "delayOnInit", "type": "Boolean", "value": True}],
                 "notes": "Require ten continuous minutes of physical position mismatch."},
                {"name": f"{prefix}_clear", "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
                 "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "level")],
                 "parameters": [{"name": "k", "type": "Integer", "value": 0}],
                 "notes": "No damper tracking alarm."},
                {"name": f"{prefix}_level_three", "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
                 "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "level")],
                 "parameters": [{"name": "k", "type": "Integer", "value": 3}],
                 "notes": "Teaching Level 3 damper position tracking alarm."},
                {"name": f"{prefix}_select", "block_type": "Buildings.Controls.OBC.CDL.Integers.Switch",
                 "category": "elementary",
                 "inputs": [port("u1", "Integer", "level"), port("u2", "Boolean"), port("u3", "Integer", "level")],
                 "outputs": [port("y", "Integer", "level")],
                 "notes": "Publish alarm level from the timed CDL condition."},
            ])
            for block in ahu["blocks"]:
                if block["name"] == f"{prefix}_error_high":
                    for connector in block["inputs"]:
                        connector["unit"] = "fraction (0–1)"
            command = "outdoor_damper_physical_limit.y" if damper == "outdoor" else "return_damper_physical_limit.y"
            measured = f"actual_{damper}_damper_position"
            ahu["connections"].extend(wire(source, target) for source, target in [
                (f"{command}", f"{prefix}_high_error.u1"),
                (measured, f"{prefix}_high_error.u2"),
                (measured, f"{prefix}_low_error.u1"),
                (f"{command}", f"{prefix}_low_error.u2"),
                (f"{prefix}_high_error.y", f"{prefix}_absolute_error.u1"),
                (f"{prefix}_low_error.y", f"{prefix}_absolute_error.u2"),
                (f"{prefix}_absolute_error.y", f"{prefix}_error_high.u1"),
                (f"{prefix}_tolerance.y", f"{prefix}_error_high.u2"),
                (f"{prefix}_error_high.y", f"{prefix}_condition.u1"),
                ("supply_fan_proven", f"{prefix}_condition.u2"),
                (f"{prefix}_condition.y", f"{prefix}_delay.u"),
                (f"{prefix}_level_three.y", f"{prefix}_select.u1"),
                (f"{prefix}_delay.y", f"{prefix}_select.u2"),
                (f"{prefix}_clear.y", f"{prefix}_select.u3"),
                (f"{prefix}_select.y", f"{damper}_damper_tracking_alarm_level"),
            ])
            output = f"{damper}_damper_tracking_alarm_level"
            ahu["outputs"].append(port(output, "Integer", "level", f"{label} Damper Tracking Alarm", "Alarm"))
            root_output = f"{name}_{output}"
            graph["outputs"].append(port(root_output, "Integer", "level",
                                         f"AHU {index} {label} Damper Tracking Alarm", "Alarm"))
            graph["connections"].append(wire(f"{name}.{output}", root_output))
        ahu["notes"] += " Physical outdoor and return damper position sensors feed editable CDL tracking alarms."
    graph["notes"] += " AHU intake physics uses measured damper positions; CDL diagnoses command mismatch."
