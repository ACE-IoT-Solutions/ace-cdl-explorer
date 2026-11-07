"""Editable CDL unoccupied group modes and zone freeze setback."""


def add_group_unoccupied_modes(graph, port, constant, binary, switch):
    def wire(source, target):
        return {"from": source, "to": target}

    def comparison(name, kind, unit, note):
        return {"name": name, "block_type": kind, "category": "elementary",
                "inputs": [port("u1", unit=unit), port("u2", unit=unit)],
                "outputs": [port("y", "Boolean")], "notes": note}

    def arithmetic(name, kind, input_unit, output_unit, note):
        return {"name": name, "block_type": kind, "category": "elementary",
                "inputs": [port("u1", unit=input_unit), port("u2", unit=input_unit)],
                "outputs": [port("y", unit=output_unit)], "notes": note}

    def temperature_margin(name, kind, note):
        return {"name": name, "block_type": kind, "category": "elementary",
                "inputs": [port("u1", unit="°C"), port("u2", unit="K")],
                "outputs": [port("y", unit="°C")], "notes": note}

    def logic(name, kind, note):
        return binary(name, kind, "Boolean", "Boolean", note)

    def latch(name, note):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Logical.Latch",
                "category": "elementary", "inputs": [port("u", "Boolean"), port("clr", "Boolean")],
                "outputs": [port("y", "Boolean")], "notes": note}

    def inverse(name, note):
        return {"name": name, "block_type": "Not", "category": "elementary",
                "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
                "notes": note}

    def delay(name, start):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Discrete.UnitDelay",
                "category": "elementary", "inputs": [port("u", unit="°C")],
                "outputs": [port("y", unit="°C")],
                "parameters": [{"name": "samplePeriod", "value": 60.0, "unit": "s"},
                               {"name": "y_start", "value": start, "unit": "°C"}],
                "notes": "Sample the editable VAV unoccupied target to break the group-to-zone dependency cycle."}

    for group_index in (1, 2):
        name = f"zone_group_{group_index}"
        group = next(block for block in graph["blocks"] if block["name"] == name)
        blocks, edges = group["blocks"], group["connections"]
        group["inputs"].append(port("scheduled_occupied", "Boolean"))
        for local in range(1, 6):
            group["inputs"].extend([port(f"unoccupied_heat_{local}", unit="°C"),
                                    port(f"unoccupied_cool_{local}", unit="°C")])
        group["outputs"].extend([port("setback", "Boolean"), port("setup", "Boolean"),
                                  port("freeze_setback", "Boolean"),
                                  port("freeze_alarm_level", "Integer", "level")])
        blocks.extend([
            constant("setback_end_margin", 1.0, "K", "End setback only when every zone exceeds its unoccupied heating target by 1 K."),
            constant("setup_end_margin", 1.0, "K", "End setup only when every zone is below its unoccupied cooling target by 1 K."),
            constant("freeze_setback_on", 4.0, "°C", "Any zone below this temperature starts freeze setback."),
            constant("freeze_setback_off", 7.0, "°C", "Every zone above this temperature clears freeze setback."),
            comparison("average_cold", "Less", "°C", "The group average is below its average unoccupied heating target."),
            comparison("average_hot", "Greater", "°C", "The group average is above its average unoccupied cooling target."),
            logic("setback_start", "Or", "All five zones cold or the group average cold starts setback."),
            logic("setup_start", "Or", "All five zones hot or the group average hot starts setup."),
            latch("setback_hold", "Hold setback until every zone exceeds its heating target by the editable margin."),
            latch("setup_hold", "Hold setup until every zone falls below its cooling target by the editable margin."),
            comparison("freeze_start", "Less", "°C", "Start freeze setback when the coldest zone falls below 4 °C."),
            comparison("freeze_end", "Greater", "°C", "End freeze setback when the coldest zone exceeds 7 °C."),
            latch("freeze_hold", "Latch zone freeze setback until all five zones recover."),
            inverse("schedule_unoccupied_group", "Only evaluate unoccupied modes outside the occupied schedule."),
            inverse("not_preconditioning_group", "Warm-up and cooldown take priority over unoccupied modes."),
            logic("unoccupied_allowed", "And", "No schedule or optimal start is active."),
            logic("freeze_active", "And", "Report freeze setback only in unoccupied mode."),
            inverse("not_freeze_group", "Freeze setback has priority over ordinary setback and setup."),
            logic("ordinary_allowed", "And", "Permit ordinary setback or setup when no freeze mode is active."),
            logic("setback_active", "And", "Apply the latched group setback decision."),
            inverse("not_setback_group", "Ordinary setback takes priority over setup."),
            logic("setup_allowed", "And", "Exclude setup while setback is active."),
            logic("setup_active", "And", "Apply the latched group setup decision."),
            {"name": "freeze_alarm_three", "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
             "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "level")],
             "parameters": [{"name": "k", "type": "Integer", "value": 3}], "notes": "Level 3 zone freeze setback alarm."},
            {"name": "freeze_alarm_clear", "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
             "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "level")],
             "parameters": [{"name": "k", "type": "Integer", "value": 0}], "notes": "No zone freeze alarm."},
            {"name": "freeze_alarm_select", "block_type": "Buildings.Controls.OBC.CDL.Integers.Switch",
             "category": "elementary", "inputs": [port("u1", "Integer", "level"), port("u2", "Boolean"),
                                                port("u3", "Integer", "level")],
             "outputs": [port("y", "Integer", "level")], "notes": "Expose Level 3 during group freeze setback."},
        ])
        accumulators = {}
        for local in range(1, 6):
            room = f"room_{local}"
            heat = f"unoccupied_heat_{local}"
            cool = f"unoccupied_cool_{local}"
            blocks.extend([
                comparison(f"cold_{local}", "Less", "°C", f"Zone {local} below its unoccupied heating target."),
                comparison(f"hot_{local}", "Greater", "°C", f"Zone {local} above its unoccupied cooling target."),
                temperature_margin(f"warm_end_target_{local}", "Add", "Heating target plus the setback exit margin."),
                temperature_margin(f"cool_end_target_{local}", "Subtract", "Cooling target minus the setup exit margin."),
                comparison(f"warm_end_{local}", "Greater", "°C", "Zone recovered above its setback exit target."),
                comparison(f"cool_end_{local}", "Less", "°C", "Zone recovered below its setup exit target."),
            ])
            edges.extend([wire(room, f"cold_{local}.u1"), wire(heat, f"cold_{local}.u2"),
                          wire(room, f"hot_{local}.u1"), wire(cool, f"hot_{local}.u2"),
                          wire(heat, f"warm_end_target_{local}.u1"),
                          wire("setback_end_margin.y", f"warm_end_target_{local}.u2"),
                          wire(cool, f"cool_end_target_{local}.u1"),
                          wire("setup_end_margin.y", f"cool_end_target_{local}.u2"),
                          wire(room, f"warm_end_{local}.u1"),
                          wire(f"warm_end_target_{local}.y", f"warm_end_{local}.u2"),
                          wire(room, f"cool_end_{local}.u1"),
                          wire(f"cool_end_target_{local}.y", f"cool_end_{local}.u2")])
            terms = {"all_cold": f"cold_{local}.y", "all_hot": f"hot_{local}.y",
                     "all_warm": f"warm_end_{local}.y", "all_cool": f"cool_end_{local}.y",
                     "sum_room": room, "sum_heat": heat, "sum_cool": cool,
                     "min_room": room}
            if local == 1:
                accumulators = terms
            else:
                for stem, term in terms.items():
                    kind = "And" if stem.startswith("all_") else "Min" if stem == "min_room" else "Add"
                    unit = "Boolean" if stem.startswith("all_") else "°C"
                    block_name = f"{stem}_{local}"
                    blocks.append(logic(block_name, kind, "Aggregate all five group zones.") if unit == "Boolean" else
                                  arithmetic(block_name, kind, unit, unit, "Aggregate all five group zones."))
                    edges.extend([wire(accumulators[stem], f"{block_name}.u1"),
                                  wire(term, f"{block_name}.u2")])
                    accumulators[stem] = f"{block_name}.y"
        edges.extend([wire(accumulators["sum_room"], "average_cold.u1"),
                      wire(accumulators["sum_heat"], "average_cold.u2"),
                      wire(accumulators["sum_room"], "average_hot.u1"),
                      wire(accumulators["sum_cool"], "average_hot.u2"),
                      wire(accumulators["all_cold"], "setback_start.u1"),
                      wire("average_cold.y", "setback_start.u2"),
                      wire(accumulators["all_hot"], "setup_start.u1"),
                      wire("average_hot.y", "setup_start.u2"),
                      wire("setback_start.y", "setback_hold.u"),
                      wire(accumulators["all_warm"], "setback_hold.clr"),
                      wire("setup_start.y", "setup_hold.u"),
                      wire(accumulators["all_cool"], "setup_hold.clr"),
                      wire(accumulators["min_room"], "freeze_start.u1"),
                      wire("freeze_setback_on.y", "freeze_start.u2"),
                      wire(accumulators["min_room"], "freeze_end.u1"),
                      wire("freeze_setback_off.y", "freeze_end.u2"),
                      wire("freeze_start.y", "freeze_hold.u"),
                      wire("freeze_end.y", "freeze_hold.clr"),
                      wire("scheduled_occupied", "schedule_unoccupied_group.u"),
                      wire("any_preconditioning.y", "not_preconditioning_group.u"),
                      wire("schedule_unoccupied_group.y", "unoccupied_allowed.u1"),
                      wire("not_preconditioning_group.y", "unoccupied_allowed.u2"),
                      wire("freeze_hold.y", "freeze_active.u1"),
                      wire("unoccupied_allowed.y", "freeze_active.u2"),
                      wire("freeze_active.y", "freeze_setback"),
                      wire("freeze_active.y", "not_freeze_group.u"),
                      wire("unoccupied_allowed.y", "ordinary_allowed.u1"),
                      wire("not_freeze_group.y", "ordinary_allowed.u2"),
                      wire("setback_hold.y", "setback_active.u1"),
                      wire("ordinary_allowed.y", "setback_active.u2"),
                      wire("setback_active.y", "setback"),
                      wire("setback_active.y", "not_setback_group.u"),
                      wire("ordinary_allowed.y", "setup_allowed.u1"),
                      wire("not_setback_group.y", "setup_allowed.u2"),
                      wire("setup_hold.y", "setup_active.u1"),
                      wire("setup_allowed.y", "setup_active.u2"),
                      wire("setup_active.y", "setup"),
                      wire("freeze_alarm_three.y", "freeze_alarm_select.u1"),
                      wire("freeze_active.y", "freeze_alarm_select.u2"),
                      wire("freeze_alarm_clear.y", "freeze_alarm_select.u3"),
                      wire("freeze_alarm_select.y", "freeze_alarm_level")])
        group["description"] = "Five-zone CDL operating-mode coordinator for one AHU."
        group["notes"] += " Editable latches coordinate ordinary setback/setup and Level 3 zone freeze setback across the five zones."
        graph["connections"].append(wire("schedule_occupied", f"{name}.scheduled_occupied"))
        for output in ("setback", "setup", "freeze_setback", "freeze_alarm_level"):
            port_type = "Integer" if output == "freeze_alarm_level" else "Boolean"
            point = f"ahu_{group_index}_group_{output}"
            graph["outputs"].append(port(point, port_type, "level" if port_type == "Integer" else "unitless",
                                         f"AHU {group_index} Zone Group {output.replace('_', ' ').title()}",
                                         "Point"))
            graph["connections"].append(wire(f"{name}.{output}", point))
        for local in range(1, 6):
            zone_index = (group_index - 1) * 5 + local
            zone_name = f"zone_{zone_index:02}"
            zone = next(block for block in graph["blocks"] if block["name"] == zone_name)
            zone["outputs"].extend([port("unoccupied_heat_target", unit="°C"),
                                    port("unoccupied_cool_target", unit="°C")])
            next(point for point in zone["outputs"] if point["name"] == "conditioning_state")["description"] = (
                "Teaching codes: 0 unoccupied, 1 occupied, 2 sensor standby, 3 warm-up, "
                "4 cooldown, 5 setup, 6 setback, 7 zone freeze setback")
            zone["inputs"].extend([port("group_setback", "Boolean"), port("group_setup", "Boolean"),
                                   port("group_freeze_setback", "Boolean")])
            zone["blocks"].extend([
                constant("unoccupied_heat_off", -50.0, "°C", "Disable ordinary unoccupied heating outside group setback/freeze."),
                constant("unoccupied_cool_off", 80.0, "°C", "Disable ordinary unoccupied cooling outside group setup."),
                logic("group_heat_allowed", "Or", "Permit unoccupied heat during group setback or freeze protection."),
                switch("unoccupied_heat_enabled", "°C", "Select group setback target or off target."),
                switch("unoccupied_cool_enabled", "°C", "Select group setup target or off target."),
                {"name": "mode_freeze_setback", "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
                 "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "count")],
                 "parameters": [{"name": "k", "type": "Integer", "value": 7}],
                 "notes": "Local teaching code 7 for group zone-freeze setback."},
                {"name": "mode_select_freeze_setback", "block_type": "Buildings.Controls.OBC.CDL.Integers.Switch",
                 "category": "elementary", "inputs": [port("u1", "Integer", "count"),
                                                    port("u2", "Boolean"), port("u3", "Integer", "count")],
                 "outputs": [port("y", "Integer", "count")],
                 "notes": "Show group freeze setback before ordinary setback/setup."},
            ])
            zone_edges = zone["connections"]
            replacements = {("unoccupied_heat.y", "heat_setpoint_active.u2"): "unoccupied_heat_enabled.y",
                            ("unoccupied_cool.y", "cool_setpoint_active.u2"): "unoccupied_cool_enabled.y",
                            ("setback_needed.y", "mode_select_setback.u2"): "group_setback",
                            ("setup_needed.y", "mode_select_setup.u2"): "group_setup",
                            ("mode_select_setup.y", "mode_select_cooldown.u3"): "mode_select_freeze_setback.y"}
            for edge in zone_edges:
                edge["from"] = replacements.get((edge["from"], edge["to"]), edge["from"])
            zone_edges.extend([wire("unoccupied_heat.y", "unoccupied_heat_target"),
                               wire("unoccupied_cool.y", "unoccupied_cool_target"),
                               wire("unoccupied_heat.y", "unoccupied_heat_enabled.u1"),
                               wire("unoccupied_heat_off.y", "unoccupied_heat_enabled.u2"),
                               wire("group_setback", "group_heat_allowed.u1"),
                               wire("group_freeze_setback", "group_heat_allowed.u2"),
                               wire("group_heat_allowed.y", "unoccupied_heat_enabled.u3"),
                               wire("unoccupied_cool.y", "unoccupied_cool_enabled.u1"),
                               wire("unoccupied_cool_off.y", "unoccupied_cool_enabled.u2"),
                               wire("group_setup", "unoccupied_cool_enabled.u3"),
                               wire("mode_freeze_setback.y", "mode_select_freeze_setback.u1"),
                               wire("group_freeze_setback", "mode_select_freeze_setback.u2"),
                               wire("mode_select_setup.y", "mode_select_freeze_setback.u3")])
            for stem, start in (("heat", 18.0), ("cool", 32.0)):
                previous = f"{zone_name}_unoccupied_{stem}_previous"
                graph["blocks"].append(delay(previous, start))
                graph["connections"].extend([
                    wire(f"{zone_name}.unoccupied_{stem}_target", f"{previous}.u"),
                    wire(f"{previous}.y", f"{name}.unoccupied_{stem}_{local}"),
                ])
            graph["connections"].extend([
                wire(f"{name}.setback", f"{zone_name}.group_setback"),
                wire(f"{name}.setup", f"{zone_name}.group_setup"),
                wire(f"{name}.freeze_setback", f"{zone_name}.group_freeze_setback"),
            ])
            zone["notes"] += " Unoccupied heating/cooling is commanded by the shared CDL group setback/setup/freeze decisions."
    graph["notes"] += " Two five-zone CDL group controllers now command latched setback, setup, and zone freeze setback."
