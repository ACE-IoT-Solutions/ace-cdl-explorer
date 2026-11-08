"""Use pinned OBC OperationMode as the decision source for both five-zone groups."""

from obc_operation_mode import MODES, operation_mode


def add_obc_operation_mode_to_groups(graph, port):
    def wire(source, target):
        return {"from": source, "to": target}

    def integer_block(name, kind, inputs, outputs, note, parameters=()):
        return {"name": name, "block_type": f"Buildings.Controls.OBC.CDL.{kind}",
                "category": "elementary",
                "inputs": [port(label, typ) for label, typ in inputs],
                "outputs": [port(label, typ) for label, typ in outputs],
                "parameters": [{"name": key, "type": "Integer", "value": value}
                               for key, value in parameters], "notes": note}

    for index in (1, 2):
        group_name = f"zone_group_{index}"
        group = next(block for block in graph["blocks"] if block["name"] == group_name)
        blocks, edges = group["blocks"], group["connections"]
        mode = operation_mode(n_zones=5, freeze_on_k=4.0, freeze_off_k=7.0)
        mode["name"] = "obc_operation_mode"
        mode["notes"] += " This instance uses °C for TZonMin and editable 4/7 °C freeze thresholds."
        next(input_port for input_port in mode["inputs"] if input_port["name"] == "TZonMin")["unit"] = "°C"
        blocks.append(mode)
        group["outputs"].append(port("operation_mode", "Integer", "code",
                                     "OBC G36 Zone Group Operation Mode", "Point"))
        edges.extend(wire(source, f"obc_operation_mode.{target}") for source, target in (
            ("group_occupied.y", "u1Occ"),
            ("seconds_until_occupied", "tNexOcc"),
            ("optimal_cooldown_longest_5.y", "maxCooDowTim"),
            ("cooldown_status_any_5.y", "u1HigOccCoo"),
            ("optimal_warmup_longest_5.y", "maxWarUpTim"),
            ("warmup_status_any_5.y", "u1OccHeaHig"),
            ("average_cold.y", "u1SetBac"),
            ("all_warm_5.y", "u1EndSetBac"),
            ("min_room_5.y", "TZonMin"),
            ("average_hot.y", "u1SetUp"),
            ("all_cool_5.y", "u1EndSetUp"),
        ))
        for stem, suffix in (("open", "uOpeWin"), ("cold", "totColZon"),
                             ("hot", "totHotZon")):
            previous = None
            for zone in range(1, 6):
                source = (f"window_closed_{zone}" if stem == "open" else
                          f"zone_status_{zone}.{'yUnoHeaHig' if stem == 'cold' else 'yHigUnoCoo'}")
                if stem == "open":
                    invert = f"mode_window_open_{zone}"
                    blocks.append(integer_block(invert, "Logical.Not",
                                                (("u", "Boolean"),), (("y", "Boolean"),),
                                                "Count an open window as one."))
                    edges.append(wire(source, f"{invert}.u"))
                    source = f"{invert}.y"
                conversion = f"mode_{stem}_count_{zone}"
                blocks.append(integer_block(conversion, "Conversions.BooleanToInteger",
                                            (("u", "Boolean"),), (("y", "Integer"),),
                                            f"Convert zone {zone} {stem} status to an integer count."))
                edges.append(wire(source, f"{conversion}.u"))
                if previous is None:
                    previous = f"{conversion}.y"
                else:
                    summer = f"mode_{stem}_sum_{zone}"
                    blocks.append(integer_block(summer, "Integers.Add",
                                                (("u1", "Integer"), ("u2", "Integer")),
                                                (("y", "Integer"),),
                                                f"Count {stem} conditions through zone {zone}."))
                    edges.extend((wire(previous, f"{summer}.u1"),
                                  wire(f"{conversion}.y", f"{summer}.u2")))
                    previous = f"{summer}.y"
            edges.append(wire(previous, f"obc_operation_mode.{suffix}"))
        graph["outputs"].append(port(f"ahu_{index}_group_operation_mode", "Integer", "code",
                                     f"AHU {index} OBC Zone Group Operation Mode", "Point"))
        graph["connections"].append(wire(f"{group_name}.operation_mode",
                                         f"ahu_{index}_group_operation_mode"))
        blocks.extend([
            integer_block("mode_not_cool_request", "Logical.Not",
                          (("u", "Boolean"),), (("y", "Boolean"),),
                          "A warm-up request can proceed when no cool-down is requested."),
            integer_block("mode_warm_priority", "Logical.Or",
                          (("u1", "Boolean"), ("u2", "Boolean")), (("y", "Boolean"),),
                          "When both recovery modes request, choose the longer warm-up prediction."),
            integer_block("mode_warm_selected", "Logical.And",
                          (("u1", "Boolean"), ("u2", "Boolean")), (("y", "Boolean"),),
                          "Choose OBC warm-up request when it has recovery priority."),
            integer_block("mode_not_warm_selected", "Logical.Not",
                          (("u", "Boolean"),), (("y", "Boolean"),),
                          "A selected warm-up excludes simultaneous cool-down."),
            integer_block("mode_cool_selected", "Logical.And",
                          (("u1", "Boolean"), ("u2", "Boolean")), (("y", "Boolean"),),
                          "Choose OBC cool-down request when warm-up is not selected."),
            integer_block("mode_warm_permitted", "Logical.And",
                          (("u1", "Boolean"), ("u2", "Boolean")), (("y", "Boolean"),),
                          "The editable calendar lookahead must permit warm-up."),
            integer_block("mode_cool_permitted", "Logical.And",
                          (("u1", "Boolean"), ("u2", "Boolean")), (("y", "Boolean"),),
                          "The editable calendar lookahead must permit cool-down."),
        ])
        edges.extend(wire(source, target) for source, target in (
            ("obc_operation_mode.yCoolDownRequest", "mode_not_cool_request.u"),
            ("mode_not_cool_request.y", "mode_warm_priority.u1"),
            ("warmup_longer.y", "mode_warm_priority.u2"),
            ("obc_operation_mode.yWarmUpRequest", "mode_warm_selected.u1"),
            ("mode_warm_priority.y", "mode_warm_selected.u2"),
            ("mode_warm_selected.y", "mode_not_warm_selected.u"),
            ("obc_operation_mode.yCoolDownRequest", "mode_cool_selected.u1"),
            ("mode_not_warm_selected.y", "mode_cool_selected.u2"),
            ("mode_warm_selected.y", "mode_warm_permitted.u1"),
            ("lookahead_allowed", "mode_warm_permitted.u2"),
            ("mode_cool_selected.y", "mode_cool_permitted.u1"),
            ("lookahead_allowed", "mode_cool_permitted.u2"),
        ))
        replacement = {}
        for label, code, old in (("warmup", MODES["warmUp"], "warmup_selected.y"),
                                 ("cooldown", MODES["coolDown"], "cooldown_selected.y"),
                                 ("setback", MODES["setBack"], "setback_active.y"),
                                 ("setup", MODES["setUp"], "setup_active.y"),
                                 ("freeze", MODES["freezeProtection"], "freeze_active.y")):
            constant_name = f"mode_{label}_code"
            compare_name = f"mode_is_{label}"
            blocks.append(integer_block(constant_name, "Integers.Sources.Constant", (),
                                        (("y", "Integer"),), f"OBC mode code {code}.",
                                        (("k", code),)))
            blocks.append(integer_block(compare_name, "Integers.Equal",
                                        (("u1", "Integer"), ("u2", "Integer")),
                                        (("y", "Boolean"),),
                                        f"The OBC mode selects {label}."))
            edges.append(wire(f"{constant_name}.y", f"{compare_name}.u2"))
            replacement[old] = f"{compare_name}.y"
        for label, code in (("occupied", MODES["occupied"]),
                            ("unoccupied", MODES["unoccupied"])):
            blocks.append(integer_block(f"mode_{label}_code", "Integers.Sources.Constant", (),
                                        (("y", "Integer"),), f"OBC mode code {code}.",
                                        (("k", code),)))
        previous = "mode_unoccupied_code.y"
        for label, request in (
            ("setup", "obc_operation_mode.ySetUpRequest"),
            ("setback", "obc_operation_mode.ySetBackRequest"),
            ("freeze", "obc_operation_mode.yFreezeProtectionRequest"),
            ("warmup", "mode_warm_permitted.y"),
            ("cooldown", "mode_cool_permitted.y"),
            ("occupied", "group_occupied.y"),
        ):
            selector = f"mode_select_{label}"
            blocks.append(integer_block(selector, "Integers.Switch",
                                        (("u1", "Integer"), ("u2", "Boolean"),
                                         ("u3", "Integer")), (("y", "Integer"),),
                                        f"Give {label} priority over lower operating modes."))
            edges.extend((wire(f"mode_{label}_code.y", f"{selector}.u1"),
                          wire(request, f"{selector}.u2"),
                          wire(previous, f"{selector}.u3")))
            previous = f"{selector}.y"
        edges.append(wire(previous, "operation_mode"))
        edges.extend(wire(previous, f"mode_is_{label}.u1") for label in
                     ("warmup", "cooldown", "setback", "setup", "freeze"))
        active_targets = {"warmup", "cooldown", "setback", "setup", "freeze_setback",
                          "any_preconditioning.u1", "any_preconditioning.u2",
                          "optimal_warmup_active.u", "optimal_cooldown_active.u",
                          "freeze_alarm_select.u2"}
        for edge in edges:
            if edge["to"] in active_targets and edge["from"] in replacement:
                edge["from"] = replacement[edge["from"]]
        group["notes"] += " The OBC OperationMode composite supplies the mode requests. Editable CDL priority switches select one mode when the reference block requests simultaneous modes; integer counts and thresholds remain editable."
