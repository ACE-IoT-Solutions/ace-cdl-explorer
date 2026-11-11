"""Expose physical AHU coil-valve position to editable CDL diagnostics."""


def add_ahu_coil_valve_feedback(graph, port, constant, binary):
    def wire(source, target):
        return {"from": source, "to": target}

    for index in (1, 2):
        name = f"ahu_{index}"
        ahu = next(block for block in graph["blocks"] if block["name"] == name)
        for service in ("heating", "cooling"):
            prefix = f"{service}_coil_valve_tracking"
            sensor = f"actual_{service}_coil_valve_position"
            root_sensor = f"{name}_{sensor}"
            alarm = f"{prefix}_alarm_level"
            command_output = f"{service}_coil_valve_position"
            command_sources = [edge["from"] for edge in ahu["connections"]
                               if edge["to"] == command_output]
            assert len(command_sources) == 1, f"Expected one CDL driver for {name}.{command_output}"
            command = command_sources[0]
            graph["inputs"].append(port(
                root_sensor, unit="fraction (0–1)",
                label=f"AHU {index} {service.title()} Coil Valve Position Sensor",
                brick="Valve_Position_Sensor",
                description="Physical valve position measured after the prior simulation step."))
            ahu["inputs"].append(port(
                sensor, unit="fraction (0–1)",
                label=f"{service.title()} Coil Valve Position Sensor",
                brick="Valve_Position_Sensor"))
            graph["connections"].append(wire(root_sensor, f"{name}.{sensor}"))
            if graph.get("brick_model"):
                graph["brick_model"]["entities"].append({
                    "id": root_sensor, "class": "brick:Valve_Position_Sensor",
                    "label": f"AHU {index} {service} coil valve position", "connector": root_sensor,
                })
                graph["brick_model"]["relationships"].append({
                    "subject": f"{name}_equipment", "predicate": "brick:hasPoint",
                    "object": root_sensor,
                })
            ahu["blocks"].extend([
                {"name": f"{prefix}_command_minus_actual", "block_type": "Subtract",
                 "category": "elementary",
                 "inputs": [port("u1", unit="fraction (0–1)"), port("u2", unit="fraction (0–1)")],
                 "outputs": [port("y", unit="fraction (0–1)")],
                 "notes": "CDL command minus measured physical valve position."},
                {"name": f"{prefix}_actual_minus_command", "block_type": "Subtract",
                 "category": "elementary",
                 "inputs": [port("u1", unit="fraction (0–1)"), port("u2", unit="fraction (0–1)")],
                 "outputs": [port("y", unit="fraction (0–1)")],
                 "notes": "Measured physical valve position minus CDL command."},
                {"name": f"{prefix}_absolute_error", "block_type": "Max", "category": "elementary",
                 "inputs": [port("u1", unit="fraction (0–1)"), port("u2", unit="fraction (0–1)")],
                 "outputs": [port("y", unit="fraction (0–1)")],
                 "notes": "Absolute mismatch in valve opening."},
                constant(f"{prefix}_tolerance", 0.25, "fraction (0–1)",
                         "Editable teaching tolerance of 25 percentage points."),
                {"name": f"{prefix}_error_high", "block_type": "Greater", "category": "elementary",
                 "inputs": [port("u1", unit="fraction (0–1)"), port("u2", unit="fraction (0–1)")],
                 "outputs": [port("y", "Boolean")],
                 "notes": "Valve position mismatch exceeds tolerance."},
                binary(f"{prefix}_condition", "And", "Boolean", "Boolean",
                       "Require physical fan proof during a valve mismatch."),
                {"name": f"{prefix}_delay", "block_type": "TrueDelay", "category": "elementary",
                 "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
                 "parameters": [{"name": "delayTime", "value": 600.0, "unit": "s"},
                                {"name": "delayOnInit", "type": "Boolean", "value": True}],
                 "notes": "Require ten continuous minutes of command-to-position mismatch."},
                {"name": f"{prefix}_clear", "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
                 "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "level")],
                 "parameters": [{"name": "k", "type": "Integer", "value": 0}],
                 "notes": "No coil-valve position alarm."},
                {"name": f"{prefix}_level_three", "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
                 "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "level")],
                 "parameters": [{"name": "k", "type": "Integer", "value": 3}],
                 "notes": "Teaching Level 3 position-tracking alarm; priority is not G36 verified."},
                {"name": f"{prefix}_select", "block_type": "Buildings.Controls.OBC.CDL.Integers.Switch",
                 "category": "elementary",
                 "inputs": [port("u1", "Integer", "level"), port("u2", "Boolean"),
                            port("u3", "Integer", "level")],
                 "outputs": [port("y", "Integer", "level")],
                 "notes": "Publish the CDL-timed valve mismatch alarm."},
            ])
            ahu["connections"].extend(wire(source, target) for source, target in [
                (command, f"{prefix}_command_minus_actual.u1"),
                (sensor, f"{prefix}_command_minus_actual.u2"),
                (sensor, f"{prefix}_actual_minus_command.u1"),
                (command, f"{prefix}_actual_minus_command.u2"),
                (f"{prefix}_command_minus_actual.y", f"{prefix}_absolute_error.u1"),
                (f"{prefix}_actual_minus_command.y", f"{prefix}_absolute_error.u2"),
                (f"{prefix}_absolute_error.y", f"{prefix}_error_high.u1"),
                (f"{prefix}_tolerance.y", f"{prefix}_error_high.u2"),
                (f"{prefix}_error_high.y", f"{prefix}_condition.u1"),
                ("supply_fan_proven", f"{prefix}_condition.u2"),
                (f"{prefix}_condition.y", f"{prefix}_delay.u"),
                (f"{prefix}_level_three.y", f"{prefix}_select.u1"),
                (f"{prefix}_delay.y", f"{prefix}_select.u2"),
                (f"{prefix}_clear.y", f"{prefix}_select.u3"),
                (f"{prefix}_select.y", alarm),
            ])
            ahu["outputs"].append(port(alarm, "Integer", "level",
                                       f"{service.title()} Coil Valve Tracking Alarm", "Alarm"))
            root_alarm = f"{name}_{alarm}"
            graph["outputs"].append(port(root_alarm, "Integer", "level",
                                         f"AHU {index} {service.title()} Coil Valve Tracking Alarm", "Alarm"))
            graph["connections"].append(wire(f"{name}.{alarm}", root_alarm))
        ahu["notes"] += " Physical coil-valve position sensors feed editable CDL tracking diagnostics."
    graph["notes"] += " AHU coil-valve tracking alarms are CDL decisions based on physical position feedback."
