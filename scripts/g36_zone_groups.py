"""CDL zone-group optimal start for the two five-zone AHUs."""


def add_zone_groups(graph, port, constant, binary):
    def typed_binary(name, kind, unit, note):
        return {"name": name, "block_type": kind, "category": "elementary",
                "inputs": [port("u1", unit=unit), port("u2", unit=unit)],
                "outputs": [port("y", unit=unit) if kind in ("Max", "Min")
                            else port("y", "Boolean")],
                "notes": note}

    def delay(name, start, unit):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Discrete.UnitDelay",
                "category": "elementary", "inputs": [port("u", unit=unit)],
                "outputs": [port("y", unit=unit)],
                "parameters": [{"name": "samplePeriod", "value": 60.0, "unit": "s"},
                               {"name": "y_start", "value": start, "unit": unit}],
                "notes": "Prior occupied setpoint breaks the group-to-zone composite dependency cycle."}

    def group(index):
        name = f"zone_group_{index}"
        inputs = [port("seconds_until_occupied", unit="s"),
                  port("lookahead_allowed", "Boolean")]
        for zone in range(1, 6):
            inputs.extend([port(f"room_{zone}", unit="°C"),
                           port(f"heat_target_{zone}", unit="°C"),
                           port(f"cool_target_{zone}", unit="°C")])
        outputs = [port("warmup", "Boolean"), port("cooldown", "Boolean"),
                   port("preconditioning", "Boolean")]
        blocks = [
            constant("zero_error", 0.0, "K", "Do not estimate a start time from a satisfied zone."),
            constant("maximum_start_seconds", 10800.0, "s", "Limit group warm-up and cooldown to three hours before occupancy."),
            {"name": "warmup_seconds_per_k", "block_type": "MultiplyByParameter", "category": "elementary",
             "inputs": [port("u", unit="K")], "outputs": [port("y", unit="s")],
             "parameters": [{"name": "k", "type": "Real", "value": 3600.0, "unit": "s/K"}],
             "notes": "Editable estimated warm-up time per kelvin of the coldest zone error."},
            {"name": "cooldown_seconds_per_k", "block_type": "MultiplyByParameter", "category": "elementary",
             "inputs": [port("u", unit="K")], "outputs": [port("y", unit="s")],
             "parameters": [{"name": "k", "type": "Real", "value": 3600.0, "unit": "s/K"}],
             "notes": "Editable estimated cooldown time per kelvin of the hottest zone error."},
            typed_binary("warmup_time_capped", "Min", "s", "Never begin warm-up earlier than the configured cap."),
            typed_binary("cooldown_time_capped", "Min", "s", "Never begin cooldown earlier than the configured cap."),
            typed_binary("warmup_start", "Less", "s", "Start when time to occupancy is less than required warm-up time."),
            typed_binary("cooldown_start", "Less", "s", "Start when time to occupancy is less than required cooldown time."),
            binary("warmup_candidate", "And", "Boolean", "Boolean", "Require calendar permission and a warm-up time request."),
            binary("cooldown_candidate", "And", "Boolean", "Boolean", "Require calendar permission and a cooldown time request."),
            typed_binary("warmup_longer", "Greater", "s", "Choose the longer estimated group start need when both occur."),
            {"name": "cooldown_priority", "block_type": "Not", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "notes": "A tied estimated need selects cooldown in this teaching sequence."},
            binary("warmup_selected", "And", "Boolean", "Boolean", "Apply the group warm-up candidate when it has priority."),
            binary("cooldown_selected", "And", "Boolean", "Boolean", "Apply the group cooldown candidate when it has priority."),
            binary("any_preconditioning", "Or", "Boolean", "Boolean", "Group requests warm-up or cooldown."),
        ]
        edges = []
        previous_error = {}
        for zone in range(1, 6):
            for stem, source_a, source_b in (
                ("warmup", f"heat_target_{zone}", f"room_{zone}"),
                ("cooldown", f"room_{zone}", f"cool_target_{zone}"),
            ):
                error = f"{stem}_error_{zone}"
                blocks.append({"name": error, "block_type": "Subtract", "category": "elementary",
                               "inputs": [port("u1", unit="°C"), port("u2", unit="°C")],
                               "outputs": [port("y", unit="K")],
                               "notes": f"Zone {zone} signed temperature need for group {stem}."})
                edges.extend([{"from": source_a, "to": f"{error}.u1"},
                              {"from": source_b, "to": f"{error}.u2"}])
                if zone == 1:
                    previous_error[stem] = f"{error}.y"
                else:
                    maximum = f"{stem}_maximum_{zone}"
                    blocks.append(typed_binary(maximum, "Max", "K", "Largest zone temperature need in the group."))
                    edges.extend([{"from": previous_error[stem], "to": f"{maximum}.u1"},
                                  {"from": f"{error}.y", "to": f"{maximum}.u2"}])
                    previous_error[stem] = f"{maximum}.y"
                if zone == 5:
                    positive = f"{stem}_positive_error"
                    blocks.append(typed_binary(positive, "Max", "K", "Clamp group need at zero."))
                    edges.extend([{"from": previous_error[stem], "to": f"{positive}.u1"},
                                  {"from": "zero_error.y", "to": f"{positive}.u2"}])
        edges.extend({"from": source, "to": target} for source, target in [
            ("warmup_positive_error.y", "warmup_seconds_per_k.u"),
            ("cooldown_positive_error.y", "cooldown_seconds_per_k.u"),
            ("warmup_seconds_per_k.y", "warmup_time_capped.u1"),
            ("cooldown_seconds_per_k.y", "cooldown_time_capped.u1"),
            ("maximum_start_seconds.y", "warmup_time_capped.u2"),
            ("maximum_start_seconds.y", "cooldown_time_capped.u2"),
            ("seconds_until_occupied", "warmup_start.u1"),
            ("seconds_until_occupied", "cooldown_start.u1"),
            ("warmup_time_capped.y", "warmup_start.u2"),
            ("cooldown_time_capped.y", "cooldown_start.u2"),
            ("warmup_start.y", "warmup_candidate.u1"),
            ("cooldown_start.y", "cooldown_candidate.u1"),
            ("lookahead_allowed", "warmup_candidate.u2"),
            ("lookahead_allowed", "cooldown_candidate.u2"),
            ("warmup_time_capped.y", "warmup_longer.u1"),
            ("cooldown_time_capped.y", "warmup_longer.u2"),
            ("warmup_longer.y", "cooldown_priority.u"),
            ("warmup_candidate.y", "warmup_selected.u1"),
            ("warmup_longer.y", "warmup_selected.u2"),
            ("cooldown_candidate.y", "cooldown_selected.u1"),
            ("cooldown_priority.y", "cooldown_selected.u2"),
            ("warmup_selected.y", "warmup"),
            ("cooldown_selected.y", "cooldown"),
            ("warmup_selected.y", "any_preconditioning.u1"),
            ("cooldown_selected.y", "any_preconditioning.u2"),
            ("any_preconditioning.y", "preconditioning"),
        ])
        return {"name": name, "block_type": "ZoneGroupModeController", "category": "composite",
                "description": "Five-zone CDL optimal-start coordinator for one AHU.",
                "notes": "Uses the largest occupied-setpoint temperature error, editable seconds per kelvin, and a three-hour cap. This is a teaching subset of G36 group operation; setback, setup, freeze, windows, and learned optimal-start rates remain to be added.",
                "inputs": inputs, "outputs": outputs, "parameters": [],
                "blocks": blocks, "connections": edges}

    graph["connections"] = [edge for edge in graph["connections"]
                            if edge["to"] != "preconditioning_mode"
                            and not (edge["from"] == "preconditioning_enabled.y"
                                     and edge["to"].endswith(".preconditioning_mode"))]
    graph["blocks"].append(binary("any_group_preconditioning", "Or", "Boolean", "Boolean",
                                  "Report actual warm-up or cooldown in either five-zone group."))
    for group_index in (1, 2):
        group_name = f"zone_group_{group_index}"
        graph["blocks"].append(group(group_index))
        for output in ("warmup", "cooldown", "preconditioning"):
            graph["outputs"].append(port(f"ahu_{group_index}_group_{output}", "Boolean", "unitless",
                                         f"AHU {group_index} Zone Group {output.title()}", "Point"))
            graph["connections"].append({"from": f"{group_name}.{output}",
                                          "to": f"ahu_{group_index}_group_{output}"})
        graph["connections"].extend({"from": source, "to": target} for source, target in [
            ("seconds_until_occupied", f"{group_name}.seconds_until_occupied"),
            ("preconditioning_enabled.y", f"{group_name}.lookahead_allowed"),
            (f"{group_name}.preconditioning", f"any_group_preconditioning.u{group_index}"),
        ])
        for local_index in range(1, 6):
            zone_index = (group_index - 1) * 5 + local_index
            zone_name = f"zone_{zone_index:02}"
            zone = next(block for block in graph["blocks"] if block["name"] == zone_name)
            zone["inputs"] = [p for p in zone["inputs"] if p["name"] != "preconditioning_mode"]
            zone["inputs"].extend([port("group_warmup", "Boolean", "unitless", "Group Warm-up", "Point"),
                                   port("group_cooldown", "Boolean", "unitless", "Group Cooldown", "Point")])
            zone["outputs"].extend([port("occupied_heat_target", unit="°C", label="Occupied Heating Target", brick="Point"),
                                    port("occupied_cool_target", unit="°C", label="Occupied Cooling Target", brick="Point")])
            zone["connections"] = [edge for edge in zone["connections"]
                                   if not (edge["from"] == "preconditioning_mode"
                                           and edge["to"] in ("warmup_mode.u1", "cooldown_mode.u1"))]
            zone["connections"].extend([{"from": "group_warmup", "to": "warmup_mode.u1"},
                                        {"from": "group_cooldown", "to": "cooldown_mode.u1"},
                                        {"from": "heat_setpoint.y", "to": "occupied_heat_target"},
                                        {"from": "cool_setpoint.y", "to": "occupied_cool_target"}])
            graph["blocks"].extend([
                delay(f"{zone_name}_heat_target_previous", 21.0, "°C"),
                delay(f"{zone_name}_cool_target_previous", 24.0, "°C"),
            ])
            graph["connections"].extend({"from": source, "to": target} for source, target in [
                (f"{zone_name}_room_temp", f"{group_name}.room_{local_index}"),
                (f"{zone_name}.occupied_heat_target", f"{zone_name}_heat_target_previous.u"),
                (f"{zone_name}.occupied_cool_target", f"{zone_name}_cool_target_previous.u"),
                (f"{zone_name}_heat_target_previous.y", f"{group_name}.heat_target_{local_index}"),
                (f"{zone_name}_cool_target_previous.y", f"{group_name}.cool_target_{local_index}"),
                (f"{group_name}.warmup", f"{zone_name}.group_warmup"),
                (f"{group_name}.cooldown", f"{zone_name}.group_cooldown"),
            ])
            zone["notes"] += " Group warm-up and cooldown now come from the five-zone CDL coordinator; occupied setpoint edits feed its sampled optimal-start estimate."
    graph["connections"].append({"from": "any_group_preconditioning.y", "to": "preconditioning_mode"})
    next(port for port in graph["outputs"] if port["name"] == "preconditioning_mode")["description"] = (
        "Actual CDL warm-up or cooldown in either zone group, after the editable calendar guard and group start calculation")
    graph["notes"] += " Two five-zone CDL group controllers coordinate warm-up and cooldown using a three-hour cap and sampled VAV occupied setpoints."
