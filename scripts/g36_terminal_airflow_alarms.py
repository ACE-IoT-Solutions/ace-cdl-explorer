"""Add editable measured-airflow fault diagnosis to G36 VAV composites."""


def add_terminal_airflow_alarms(graph, port, constant, binary):
    def wire(source, target):
        return {"from": source, "to": target}

    def integer_constant(name, value, note):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
                "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "level")],
                "parameters": [{"name": "k", "type": "Integer", "value": value}], "notes": note}

    def integer_switch(name, note):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Integers.Switch",
                "category": "elementary",
                "inputs": [port("u1", "Integer", "level"), port("u2", "Boolean"),
                           port("u3", "Integer", "level")],
                "outputs": [port("y", "Integer", "level")], "notes": note}

    for index in range(1, 11):
        name = f"zone_{index:02}"
        zone = next(block for block in graph["blocks"] if block["name"] == name)
        blocks, edges = zone["blocks"], zone["connections"]
        zone["inputs"].append(port("supply_fan_proven", "Boolean", "unitless",
                                   "AHU Supply Fan Proven", "Fan_Status"))
        graph["connections"].append(wire(f"ahu_{(index - 1) // 5 + 1}_supply_fan_proven",
                                          f"{name}.supply_fan_proven"))
        blocks.extend([
            {"name": "low_flow_fan_ready", "block_type": "TrueDelay", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "delayTime", "value": 1800.0, "unit": "s"},
                            {"name": "delayOnInit", "type": "Boolean", "value": True}],
             "notes": "Suppress terminal low-flow alarms until the AHU fan has been proven running for 30 continuous minutes."},
            binary("low_flow_occupied_and_fan", "And", "Boolean", "Boolean",
                   "Only an occupied zone with proven AHU fan operation may raise a low-flow alarm."),
            {"name": "low_flow_seventy_setpoint", "block_type": "MultiplyByParameter", "category": "elementary",
             "inputs": [port("u", unit="normalized airflow")],
             "outputs": [port("y", unit="normalized airflow")],
             "parameters": [{"name": "k", "type": "Real", "value": 0.7}],
             "notes": "Level 3 fault threshold is 70% of the active primary-airflow setpoint."},
            binary("low_flow_below_seventy", "Less", note="Measured primary airflow is below 70% of setpoint."),
            binary("low_flow_eligible", "And", "Boolean", "Boolean",
                   "Check low flow only for an occupied zone with a nonzero airflow request."),
            binary("low_flow_occupied_gate", "And", "Boolean", "Boolean",
                   "Suppress the existing Level 2 low-flow delay outside occupied mode."),
            binary("low_flow_seventy_condition", "And", "Boolean", "Boolean",
                   "Eligible measured flow is below the Level 3 threshold."),
            {"name": "low_flow_seventy_delay", "block_type": "TrueDelay", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "delayTime", "value": 300.0, "unit": "s"},
                            {"name": "delayOnInit", "type": "Boolean", "value": True}],
             "notes": "Require five continuous minutes of eligible flow below 70% of the request."},
            integer_constant("low_flow_alarm_zero", 0, "No terminal low-airflow alarm."),
            integer_constant("low_flow_alarm_three", 3, "Level 3 sustained low-airflow alarm."),
            integer_constant("low_flow_alarm_two", 2, "Level 2 severe low-airflow alarm."),
            integer_switch("low_flow_alarm_moderate", "Show Level 3 after the 70% fault delay."),
            integer_switch("low_flow_alarm_priority", "Severe Level 2 fault takes precedence over Level 3."),
        ])
        edges[:] = [edge for edge in edges if (edge["from"], edge["to"]) !=
                    ("low_flow_condition.y", "low_flow_delay.u")]
        edges.extend(wire(source, target) for source, target in [
            ("supply_fan_proven", "low_flow_fan_ready.u"),
            ("local_occupied.y", "low_flow_occupied_and_fan.u1"),
            ("low_flow_fan_ready.y", "low_flow_occupied_and_fan.u2"),
            ("airflow_request_sum.y", "low_flow_seventy_setpoint.u"),
            ("measured_airflow", "low_flow_below_seventy.u1"),
            ("low_flow_seventy_setpoint.y", "low_flow_below_seventy.u2"),
            ("low_flow_occupied_and_fan.y", "low_flow_eligible.u1"),
            ("flow_requested.y", "low_flow_eligible.u2"),
            ("low_flow_occupied_and_fan.y", "low_flow_occupied_gate.u1"),
            ("low_flow_condition.y", "low_flow_occupied_gate.u2"),
            ("low_flow_occupied_gate.y", "low_flow_delay.u"),
            ("low_flow_eligible.y", "low_flow_seventy_condition.u1"),
            ("low_flow_below_seventy.y", "low_flow_seventy_condition.u2"),
            ("low_flow_seventy_condition.y", "low_flow_seventy_delay.u"),
            ("low_flow_alarm_three.y", "low_flow_alarm_moderate.u1"),
            ("low_flow_seventy_delay.y", "low_flow_alarm_moderate.u2"),
            ("low_flow_alarm_zero.y", "low_flow_alarm_moderate.u3"),
            ("low_flow_alarm_two.y", "low_flow_alarm_priority.u1"),
            ("low_flow_delay.y", "low_flow_alarm_priority.u2"),
            ("low_flow_alarm_moderate.y", "low_flow_alarm_priority.u3"),
            ("low_flow_alarm_priority.y", "low_airflow_alarm_level"),
        ])
        for block in blocks:
            if block["name"] == "low_flow_below_seventy":
                for connector in block["inputs"]:
                    connector["unit"] = "normalized airflow"
        zone["outputs"].append(port("low_airflow_alarm_level", "Integer", "level",
                                    "Low Primary Airflow Alarm", "Alarm"))
        point = f"{name}_low_airflow_alarm_level"
        graph["outputs"].append(port(point, "Integer", "level",
                                     f"Zone {index} Low Primary Airflow Alarm", "Alarm"))
        graph["connections"].append(wire(f"{name}.low_airflow_alarm_level", point))
        zone["notes"] += " Editable occupied-mode low-flow diagnosis waits for 30 minutes of AHU fan proof, then raises Level 3 below 70% or Level 2 below 50% of requested primary airflow after five minutes."
    graph["notes"] += " VAV low-airflow alarms are calculated from measured delivery in editable CDL."
