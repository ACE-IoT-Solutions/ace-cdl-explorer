"""Editable CDL local occupancy overrides and group window alarms."""


def add_group_overrides(graph, port, binary):
    def wire(source, target):
        return {"from": source, "to": target}

    def logic(name, kind, note):
        return binary(name, kind, "Boolean", "Boolean", note)

    def inverse(name, note):
        return {"name": name, "block_type": "Not", "category": "elementary",
                "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
                "notes": note}

    def integer_constant(name, value, note):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
                "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "level")],
                "parameters": [{"name": "k", "type": "Integer", "value": value}], "notes": note}

    for group_index in (1, 2):
        name = f"zone_group_{group_index}"
        group = next(block for block in graph["blocks"] if block["name"] == name)
        blocks, edges = group["blocks"], group["connections"]
        group["inputs"].extend(port(f"override_{local}", "Boolean") for local in range(1, 6))
        group["outputs"].extend([port("occupied", "Boolean"),
                                  port("window_open_alarm_level", "Integer", "level")])
        blocks.extend([
            logic("override_any_2", "Or", "Any of the first two local overrides."),
            logic("override_any_3", "Or", "Any of the first three local overrides."),
            logic("override_any_4", "Or", "Any of the first four local overrides."),
            logic("override_any_5", "Or", "Any of the five local overrides."),
            logic("group_occupied", "Or", "Scheduled occupancy or any local override occupies the AHU group."),
            inverse("not_group_occupied", "Only unoccupied groups may precondition or signal a window alarm."),
            logic("lookahead_unoccupied", "And", "Block optimal start while the group is occupied."),
            logic("window_any_2", "Or", "Any of the first two windows open."),
            logic("window_any_3", "Or", "Any of the first three windows open."),
            logic("window_any_4", "Or", "Any of the first four windows open."),
            logic("window_any_5", "Or", "Any of the five windows open."),
            logic("window_alarm_active", "And", "A window open while the group is outside occupied mode."),
            integer_constant("window_alarm_four", 4, "Level 4 open-window alarm outside occupied mode."),
            integer_constant("window_alarm_clear", 0, "Clear the open-window alarm."),
            {"name": "window_alarm_select", "block_type": "Buildings.Controls.OBC.CDL.Integers.Switch",
             "category": "elementary",
             "inputs": [port("u1", "Integer", "level"), port("u2", "Boolean"),
                        port("u3", "Integer", "level")],
             "outputs": [port("y", "Integer", "level")],
             "notes": "Expose Level 4 only for open windows outside occupied mode."},
        ])
        for edge in edges:
            if edge["from"] == "scheduled_occupied" and edge["to"] == "schedule_unoccupied_group.u":
                edge["from"] = "group_occupied.y"
            if edge["from"] == "lookahead_allowed" and edge["to"] in (
                    "warmup_candidate.u2", "cooldown_candidate.u2"):
                edge["from"] = "lookahead_unoccupied.y"
        edges.extend([wire("override_1", "override_any_2.u1"),
                      wire("override_2", "override_any_2.u2")])
        for local in range(3, 6):
            edges.extend([wire(f"override_any_{local-1}.y", f"override_any_{local}.u1"),
                          wire(f"override_{local}", f"override_any_{local}.u2")])
        edges.extend([wire("scheduled_occupied", "group_occupied.u1"),
                      wire("override_any_5.y", "group_occupied.u2"),
                      wire("group_occupied.y", "occupied"),
                      wire("group_occupied.y", "not_group_occupied.u"),
                      wire("lookahead_allowed", "lookahead_unoccupied.u1"),
                      wire("not_group_occupied.y", "lookahead_unoccupied.u2"),
                      wire("window_open_1.y", "window_any_2.u1"),
                      wire("window_open_2.y", "window_any_2.u2")])
        for local in range(3, 6):
            edges.extend([wire(f"window_any_{local-1}.y", f"window_any_{local}.u1"),
                          wire(f"window_open_{local}.y", f"window_any_{local}.u2")])
        edges.extend([wire("window_any_5.y", "window_alarm_active.u1"),
                      wire("not_group_occupied.y", "window_alarm_active.u2"),
                      wire("window_alarm_four.y", "window_alarm_select.u1"),
                      wire("window_alarm_active.y", "window_alarm_select.u2"),
                      wire("window_alarm_clear.y", "window_alarm_select.u3"),
                      wire("window_alarm_select.y", "window_open_alarm_level")])

        occupied_point = f"ahu_{group_index}_group_occupied"
        alarm_point = f"ahu_{group_index}_group_window_open_alarm_level"
        graph["outputs"].extend([
            port(occupied_point, "Boolean", "unitless", f"AHU {group_index} Group Occupied", "Status"),
            port(alarm_point, "Integer", "level", f"AHU {group_index} Window Open Alarm", "Alarm"),
        ])
        graph["connections"].extend([wire(f"{name}.occupied", occupied_point),
                                      wire(f"{name}.window_open_alarm_level", alarm_point)])
        for edge in graph["connections"]:
            if edge["from"] == "schedule_occupied" and edge["to"] == f"ahu_{group_index}.occupied_mode":
                edge["from"] = f"{name}.occupied"
        for local in range(1, 6):
            zone_index = (group_index - 1) * 5 + local
            zone_name = f"zone_{zone_index:02}"
            raw = f"{zone_name}_override"
            edge_name = f"{zone_name}_override_edge"
            hold_name = f"{zone_name}_override_hold"
            active = f"{zone_name}_override_active"
            graph["inputs"].append(port(raw, "Boolean", "unitless", f"Zone {zone_index} Local Occupancy Override",
                                        "Status", "Raw local override button; CDL times the resulting occupied request."))
            graph["outputs"].append(port(active, "Boolean", "unitless", f"Zone {zone_index} Timed Local Override",
                                         "Point", "CDL local occupied request held for the editable override duration."))
            graph["blocks"].extend([
                {"name": edge_name, "block_type": "Buildings.Controls.OBC.CDL.Logical.Edge",
                 "category": "elementary", "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
                 "notes": "One button press starts one local occupancy override episode."},
                {"name": hold_name, "block_type": "Buildings.Controls.OBC.CDL.Logical.TrueFalseHold",
                 "category": "elementary", "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
                 "parameters": [{"name": "trueHoldDuration", "value": 7200.0, "unit": "s"},
                                {"name": "falseHoldDuration", "value": 0.0, "unit": "s"}],
                 "notes": "A local button press requests occupied control for two hours by default; the duration is editable."},
            ])
            graph["connections"].extend([wire(raw, f"{edge_name}.u"),
                                          wire(f"{edge_name}.y", f"{hold_name}.u"),
                                          wire(f"{hold_name}.y", f"{name}.override_{local}"),
                                          wire(f"{hold_name}.y", f"{zone_name}.local_override"),
                                          wire(f"{hold_name}.y", active)])
            zone = next(block for block in graph["blocks"] if block["name"] == zone_name)
            zone["inputs"].append(port("local_override", "Boolean", "unitless", "Local Occupancy Override", "Status"))
            zone["blocks"].append(logic("local_occupied", "Or", "Scheduled occupancy or local override selects occupied zone control."))
            for edge in zone["connections"]:
                if edge["from"] == "scheduled_occupied":
                    edge["from"] = "local_occupied.y"
            zone["connections"].extend([wire("scheduled_occupied", "local_occupied.u1"),
                                        wire("local_override", "local_occupied.u2")])
            zone["notes"] += " A timed local occupancy override selects occupied terminal mode and makes its AHU group occupied through CDL."
        group["notes"] += " Any local override makes the group occupied; an open window outside occupied mode yields a Level 4 alarm."
    graph["notes"] += " Local override button contacts feed editable two-hour CDL holds before zone/group occupancy and Level 4 window alarms."
