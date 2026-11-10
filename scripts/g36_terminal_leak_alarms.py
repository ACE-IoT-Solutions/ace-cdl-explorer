"""Editable G36 terminal damper and reheat-valve leakage diagnosis."""


def add_terminal_leak_alarms(graph, port, constant, binary):
    def wire(source, target):
        return {"from": source, "to": target}

    def delay(name, seconds, note):
        return {"name": name, "block_type": "TrueDelay", "category": "elementary",
                "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
                "parameters": [{"name": "delayTime", "value": seconds, "unit": "s"},
                               {"name": "delayOnInit", "type": "Boolean", "value": True}],
                "notes": note}

    def level(name, value):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
                "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "level")],
                "parameters": [{"name": "k", "type": "Integer", "value": value}],
                "notes": f"Level {value} terminal leakage alarm." if value else "No terminal leakage alarm."}

    def choose(name, note):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Integers.Switch",
                "category": "elementary",
                "inputs": [port("u1", "Integer", "level"), port("u2", "Boolean"),
                           port("u3", "Integer", "level")],
                "outputs": [port("y", "Integer", "level")], "notes": note}

    for index in range(1, 11):
        name = f"zone_{index:02}"
        zone = next(block for block in graph["blocks"] if block["name"] == name)
        graph["inputs"].extend([
            port(f"{name}_actual_damper_position", unit="fraction (0–1)",
                 label=f"Zone {index} VAV Damper Position Sensor", brick="Damper_Position_Sensor",
                 description="Physical damper opening after any stuck-open fault on the previous step"),
            port(f"{name}_actual_reheat_valve_position", unit="fraction (0–1)",
                 label=f"Zone {index} Reheat Valve Position Sensor", brick="Valve_Position_Sensor",
                 description="Physical valve actuator opening; seat leakage may pass water while it reads closed"),
        ])
        zone["inputs"].append(port("actual_reheat_valve_position", unit="fraction (0–1)",
                                   label="Reheat Valve Position Sensor", brick="Valve_Position_Sensor"))
        graph["connections"].append(wire(f"{name}_actual_reheat_valve_position",
                                          f"{name}.actual_reheat_valve_position"))
        zone["blocks"].extend([
            constant("leak_damper_closed_limit", 0.05, "fraction (0–1)",
                     "Treat a damper command below 5% as closed, following the OBC default."),
            {"name": "leak_damper_command_closed", "block_type": "Less", "category": "elementary",
             "inputs": [port("u1", unit="fraction (0–1)"), port("u2", unit="fraction (0–1)")],
             "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "h", "type": "Real", "value": 0.025, "unit": "fraction (0–1)"}],
             "notes": "Terminal damper commanded closed by editable CDL."},
            binary("leak_damper_closed_and_fan", "And", "Boolean", "Boolean",
                   "The damper is commanded closed while AHU fan status is proven on."),
            binary("leak_damper_flow_high", "Greater", "Real", "Boolean",
                   "Measured terminal airflow exceeds ten percent of design cooling flow."),
            binary("leak_damper_condition", "And", "Boolean", "Boolean",
                   "A closed commanded damper still passes measurable air while the fan runs."),
            delay("leak_damper_delay", 600.0,
                  "Require ten continuous minutes of closed-command flow before Level 4."),
            constant("leak_valve_closed_limit", 0.05, "fraction (0–1)",
                     "Treat a measured valve position below 5% as closed, following the OBC default."),
            {"name": "leak_valve_position_closed", "block_type": "Less", "category": "elementary",
             "inputs": [port("u1", unit="fraction (0–1)"), port("u2", unit="fraction (0–1)")],
             "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "h", "type": "Real", "value": 0.025, "unit": "fraction (0–1)"}],
             "notes": "Measured reheat valve actuator is closed, even if its seat leaks water."},
            delay("leak_valve_closed_delay", 900.0,
                  "Require fifteen continuous minutes of closed valve, fan proof, and warm discharge."),
            {"name": "leak_valve_temperature_rise", "block_type": "Subtract", "category": "elementary",
             "inputs": [port("u1", unit="°C"), port("u2", unit="°C")],
             "outputs": [port("y", unit="K")],
             "notes": "Measured VAV discharge minus measured AHU supply temperature."},
            constant("leak_valve_rise_threshold", 3.0, "K",
                     "Leaking valve threshold: discharge more than 3 K above AHU supply."),
            {"name": "leak_valve_temperature_high", "block_type": "Greater", "category": "elementary",
             "inputs": [port("u1", unit="K"), port("u2", unit="K")],
             "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "h", "type": "Real", "value": 0.25, "unit": "K"}],
             "notes": "OBC discharge temperature rise uses 0.25 K clearing hysteresis."},
            binary("leak_valve_fan_and_closed", "And", "Boolean", "Boolean",
                   "AHU fan proven on while the measured valve is closed."),
            binary("leak_valve_condition", "And", "Boolean", "Boolean",
                   "Warm discharge, closed valve, and fan proof must persist together."),
            level("leak_alarm_clear", 0),
            level("leak_alarm_four", 4),
            choose("leak_damper_select", "Publish Level 4 for sustained damper leakage."),
            choose("leak_valve_select", "Publish Level 4 for reheat-valve seat leakage."),
        ])
        for block in zone["blocks"]:
            if block["name"] == "leak_damper_flow_high":
                for connector in block["inputs"]:
                    connector["unit"] = "normalized airflow"
                block["parameters"] = [{"name": "h", "type": "Real", "value": 0.05,
                                        "unit": "normalized airflow"}]
                block["notes"] = "Measured flow exceeds ten percent of design cooling flow with 0.05 OBC clearing hysteresis."
        zone["connections"].extend(wire(source, target) for source, target in [
            ("damper_feedback_output.y", "leak_damper_command_closed.u1"),
            ("leak_damper_closed_limit.y", "leak_damper_command_closed.u2"),
            ("leak_damper_command_closed.y", "leak_damper_closed_and_fan.u1"),
            ("supply_fan_proven", "leak_damper_closed_and_fan.u2"),
            ("measured_airflow", "leak_damper_flow_high.u1"),
            ("sensor_ten_percent.y", "leak_damper_flow_high.u2"),
            ("leak_damper_closed_and_fan.y", "leak_damper_condition.u1"),
            ("leak_damper_flow_high.y", "leak_damper_condition.u2"),
            ("leak_damper_condition.y", "leak_damper_delay.u"),
            ("actual_reheat_valve_position", "leak_valve_position_closed.u1"),
            ("leak_valve_closed_limit.y", "leak_valve_position_closed.u2"),
            ("leak_valve_position_closed.y", "leak_valve_fan_and_closed.u1"),
            ("discharge_temp", "leak_valve_temperature_rise.u1"),
            ("supply_air_temp", "leak_valve_temperature_rise.u2"),
            ("leak_valve_temperature_rise.y", "leak_valve_temperature_high.u1"),
            ("leak_valve_rise_threshold.y", "leak_valve_temperature_high.u2"),
            ("supply_fan_proven", "leak_valve_fan_and_closed.u2"),
            ("leak_valve_fan_and_closed.y", "leak_valve_condition.u1"),
            ("leak_valve_temperature_high.y", "leak_valve_condition.u2"),
            ("leak_valve_condition.y", "leak_valve_closed_delay.u"),
            ("leak_alarm_four.y", "leak_damper_select.u1"),
            ("leak_damper_delay.y", "leak_damper_select.u2"),
            ("leak_alarm_clear.y", "leak_damper_select.u3"),
            ("leak_alarm_four.y", "leak_valve_select.u1"),
            ("leak_valve_closed_delay.y", "leak_valve_select.u2"),
            ("leak_alarm_clear.y", "leak_valve_select.u3"),
            ("leak_damper_select.y", "leaking_damper_alarm_level"),
            ("leak_valve_select.y", "leaking_reheat_valve_alarm_level"),
        ])
        for suffix, label in (("leaking_damper_alarm_level", "Leaking VAV Damper Alarm"),
                              ("leaking_reheat_valve_alarm_level", "Leaking Reheat Valve Alarm")):
            zone["outputs"].append(port(suffix, "Integer", "level", label, "Alarm"))
            point = f"{name}_{suffix}"
            graph["outputs"].append(port(point, "Integer", "level", f"Zone {index} {label}", "Alarm"))
            graph["connections"].append(wire(f"{name}.{suffix}", point))
        zone["notes"] += " Editable Level 4 leakage alarms use fan status, closed damper command plus measured flow, and closed measured valve plus discharge temperature rise."
    graph["notes"] += " Physical VAV actuator positions and seat-leak behavior feed editable CDL terminal leakage alarms."
