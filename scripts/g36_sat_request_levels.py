"""Replace binary AHU SAT requests with editable VAV 0–3 request branches."""


def add_sat_request_levels(graph, port):
    def wire(source, target):
        return {"from": source, "to": target}

    def constant(name, value, unit, note):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Reals.Sources.Constant",
                "category": "elementary", "inputs": [], "outputs": [port("y", "Real", unit)],
                "parameters": [{"name": "k", "type": "Real", "value": value, "unit": unit}], "notes": note}

    def real_binary(name, kind, unit, note, output_unit=None):
        return {"name": name, "block_type": f"Buildings.Controls.OBC.CDL.Reals.{kind}",
                "category": "elementary",
                "inputs": [port("u1", "Real", unit), port("u2", "Real", unit)],
                "outputs": [port("y", "Boolean" if kind == "Greater" else "Real",
                                 None if kind == "Greater" else (output_unit or unit))], "notes": note}

    def logical(name, kind, note):
        inputs = [port("u", "Boolean")] if kind == "Not" else [port("u1", "Boolean"), port("u2", "Boolean")]
        return {"name": name, "block_type": f"Buildings.Controls.OBC.CDL.Logical.{kind}",
                "category": "elementary", "inputs": inputs,
                "outputs": [port("y", "Boolean")], "notes": note}

    def delay(name, duration, on_init, note):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Logical.TrueDelay",
                "category": "elementary", "inputs": [port("u", "Boolean")],
                "outputs": [port("y", "Boolean")],
                "parameters": [{"name": "delayTime", "type": "Real", "value": duration, "unit": "s"},
                               {"name": "delayOnInit", "type": "Boolean", "value": on_init}],
                "notes": note}

    def level(name, number):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
                "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "count")],
                "parameters": [{"name": "k", "type": "Integer", "value": number}],
                "notes": f"{number} cooling supply-air temperature reset requests."}

    def select(name, note):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Integers.Switch",
                "category": "elementary",
                "inputs": [port("u1", "Integer", "count"), port("u2", "Boolean"),
                           port("u3", "Integer", "count")],
                "outputs": [port("y", "Integer", "count")], "notes": note}

    old_suffixes = {"sat_demand_threshold", "sat_demand_high", "sat_damper_open",
                    "sat_request_condition", "sat_request_delay", "sat_request_count"}
    changed = False
    for index in range(1, 11):
        name = f"zone_{index:02}"
        zone = next(block for block in graph["blocks"] if block["name"] == name)
        if any(block["name"] == "sat_request_three" for block in zone["blocks"]):
            continue
        changed = True
        zone["blocks"].extend([
            real_binary("sat_cooling_error", "Subtract", "°C", "Zone temperature minus active cooling setpoint.", "K"),
            constant("sat_two_error_threshold", 2.0, "K", "Two requests require more than 2 K of cooling error."),
            constant("sat_three_error_threshold", 3.0, "K", "Three requests require more than 3 K of cooling error."),
            real_binary("sat_two_error_high", "Greater", "K", "Cooling error exceeds 2 K."),
            real_binary("sat_three_error_high", "Greater", "K", "Cooling error exceeds 3 K."),
            delay("sat_two_error_delay", 120.0, True, "Two continuous minutes above 2 K."),
            delay("sat_three_error_delay", 120.0, True, "Two continuous minutes above 3 K."),
            {"name": "sat_cooling_loop_high", "block_type": "Buildings.Controls.OBC.CDL.Reals.Hysteresis",
             "category": "elementary", "inputs": [port("u", "Real", "fraction (0–1)")],
             "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "uLow", "type": "Real", "value": 0.85},
                            {"name": "uHigh", "type": "Real", "value": 0.95}],
             "notes": "One request starts above 95% cooling-loop proxy and holds until below 85%."},
            {"name": "sat_previous_cooling_setpoint", "block_type": "Buildings.Controls.OBC.CDL.Discrete.UnitDelay",
             "category": "elementary", "inputs": [port("u", "Real", "°C")],
             "outputs": [port("y", "Real", "°C")],
             "parameters": [{"name": "samplePeriod", "type": "Real", "value": 120.0, "unit": "s"},
                            {"name": "y_start", "type": "Real", "value": 24.0, "unit": "°C"}],
             "notes": "Sample the active cooling setpoint every two minutes for change suppression."},
            real_binary("sat_setpoint_rise", "Subtract", "°C", "Current setpoint minus previous sample.", "K"),
            real_binary("sat_setpoint_fall", "Subtract", "°C", "Previous setpoint minus current sample.", "K"),
            constant("sat_change_threshold", 0.25, "K", "Ignore setpoint changes of 0.25 K or less."),
            real_binary("sat_setpoint_rise_detected", "Greater", "K", "Detect a material setpoint rise."),
            real_binary("sat_setpoint_fall_detected", "Greater", "K", "Detect a material setpoint fall."),
            logical("sat_setpoint_changed", "Or", "Either setpoint direction starts suppression."),
            logical("sat_setpoint_stable", "Not", "No material setpoint change this sample."),
            delay("sat_suppression_elapsed", 1800.0, False,
                  "After a change, suppress two- and three-request levels for an editable 30 minutes."),
            logical("sat_two_eligible", "And", "Two requests require sustained error and completed suppression."),
            logical("sat_three_eligible", "And", "Three requests require sustained error and completed suppression."),
            *(level(f"sat_level_{number}", number) for number in range(4)),
            select("sat_request_one", "One request while the cooling-loop proxy is high."),
            select("sat_request_two", "Two sustained-error requests take precedence."),
            select("sat_request_three", "Three sustained-error requests take precedence."),
        ])
        zone["outputs"].append(port("cooling_sat_reset_request", "Integer", "count",
                                    "Cooling Supply Air Temperature Reset Request", "Point"))
        zone["connections"].extend(wire(source, target) for source, target in [
            ("room_temp", "sat_cooling_error.u1"),
            ("window_cool_target.y", "sat_cooling_error.u2"),
            ("sat_cooling_error.y", "sat_two_error_high.u1"),
            ("sat_two_error_threshold.y", "sat_two_error_high.u2"),
            ("sat_cooling_error.y", "sat_three_error_high.u1"),
            ("sat_three_error_threshold.y", "sat_three_error_high.u2"),
            ("sat_two_error_high.y", "sat_two_error_delay.u"),
            ("sat_three_error_high.y", "sat_three_error_delay.u"),
            ("cool_demand_limit.y", "sat_cooling_loop_high.u"),
            ("window_cool_target.y", "sat_previous_cooling_setpoint.u"),
            ("window_cool_target.y", "sat_setpoint_rise.u1"),
            ("sat_previous_cooling_setpoint.y", "sat_setpoint_rise.u2"),
            ("sat_previous_cooling_setpoint.y", "sat_setpoint_fall.u1"),
            ("window_cool_target.y", "sat_setpoint_fall.u2"),
            ("sat_setpoint_rise.y", "sat_setpoint_rise_detected.u1"),
            ("sat_change_threshold.y", "sat_setpoint_rise_detected.u2"),
            ("sat_setpoint_fall.y", "sat_setpoint_fall_detected.u1"),
            ("sat_change_threshold.y", "sat_setpoint_fall_detected.u2"),
            ("sat_setpoint_rise_detected.y", "sat_setpoint_changed.u1"),
            ("sat_setpoint_fall_detected.y", "sat_setpoint_changed.u2"),
            ("sat_setpoint_changed.y", "sat_setpoint_stable.u"),
            ("sat_setpoint_stable.y", "sat_suppression_elapsed.u"),
            ("sat_two_error_delay.y", "sat_two_eligible.u1"),
            ("sat_suppression_elapsed.y", "sat_two_eligible.u2"),
            ("sat_three_error_delay.y", "sat_three_eligible.u1"),
            ("sat_suppression_elapsed.y", "sat_three_eligible.u2"),
            ("sat_level_1.y", "sat_request_one.u1"),
            ("sat_cooling_loop_high.y", "sat_request_one.u2"),
            ("sat_level_0.y", "sat_request_one.u3"),
            ("sat_level_2.y", "sat_request_two.u1"),
            ("sat_two_eligible.y", "sat_request_two.u2"),
            ("sat_request_one.y", "sat_request_two.u3"),
            ("sat_level_3.y", "sat_request_three.u1"),
            ("sat_three_eligible.y", "sat_request_three.u2"),
            ("sat_request_two.y", "sat_request_three.u3"),
            ("sat_request_three.y", "cooling_sat_reset_request"),
        ])
        graph["outputs"].append(port(f"{name}_cooling_sat_reset_request", "Integer", "count",
                                      f"Zone {index} Cooling SAT Reset Request", "Point"))
        graph["connections"].append(wire(f"{name}.cooling_sat_reset_request",
                                          f"{name}_cooling_sat_reset_request"))
        zone["notes"] += " Cooling SAT reset sends editable 0–3 requests from zone error and cooling-loop demand; higher tiers wait through setpoint-change suppression."

    for ahu in (1, 2):
        name = f"ahu_{ahu}"
        controller = next(block for block in graph["blocks"] if block["name"] == name)
        ahu_changed = False
        for index in range((ahu - 1) * 5 + 1, ahu * 5 + 1):
            zone = f"zone_{index:02}"
            if any(port_["name"] == f"{zone}_sat_request" for port_ in controller["inputs"]):
                continue
            ahu_changed = True
            former = {f"{zone}_{suffix}" for suffix in old_suffixes}
            request_source = f"{zone}_sat_request_count.y"
            controller["blocks"] = [block for block in controller["blocks"] if block["name"] not in former]
            controller["connections"] = [edge for edge in controller["connections"]
                                         if (edge["from"].split(".")[0] not in former or
                                             (edge["from"] == request_source and
                                              edge["to"].startswith("sat_request_sum_")))
                                         and edge["to"].split(".")[0] not in former]
            controller["inputs"] = [input_ for input_ in controller["inputs"]
                                    if input_["name"] not in {f"{zone}_cooling_demand", f"{zone}_damper_position"}]
            graph["connections"] = [edge for edge in graph["connections"]
                                    if edge["to"] not in {f"{name}.{zone}_cooling_demand",
                                                           f"{name}.{zone}_damper_position"}]
            controller["inputs"].append(port(f"{zone}_sat_request", "Integer", "count",
                                             f"Zone {index} Cooling SAT Reset Request", "Point"))
            graph["connections"].append(wire(f"{zone}.cooling_sat_reset_request",
                                              f"{name}.{zone}_sat_request"))
            for edge in controller["connections"]:
                if edge["from"] == request_source:
                    edge["from"] = f"{zone}_sat_request"
        if ahu_changed:
            controller["notes"] += " Typed 0–3 VAV requests enter the existing sampled SAT trim-and-respond sum."
    if changed:
        graph["notes"] += " Cooling SAT request levels and setpoint-change suppression are calculated in each editable VAV composite."
