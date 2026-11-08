"""Wire pinned OBC zone-status composites into both five-zone CDL groups."""

from obc_zone_status import zone_status


def add_obc_zone_status_to_groups(graph, binary):
    def wire(source, target):
        return {"from": source, "to": target}

    for group_index in (1, 2):
        group = next(block for block in graph["blocks"]
                     if block["name"] == f"zone_group_{group_index}")
        blocks, edges = group["blocks"], group["connections"]
        status_outputs = {"warmup": [], "cooldown": []}
        for zone in range(1, 6):
            name = f"zone_status_{zone}"
            status = zone_status()
            status["name"] = name
            status["notes"] += (
                " This building exposes temperatures in °C. All five temperature inputs use "
                "the same offset from the OBC Kelvin ports, so each Subtract result and "
                "hysteresis decision is unchanged."
            )
            for input_port in status["inputs"]:
                if input_port["name"] in ("TZon", "TOccHeaSet", "TOccCooSet",
                                          "TUnoHeaSet", "TUnoCooSet"):
                    input_port["unit"] = "°C"
            blocks.append(status)
            for source, target in (
                (f"room_{zone}", "TZon"),
                (f"heat_target_{zone}", "TOccHeaSet"),
                (f"cool_target_{zone}", "TOccCooSet"),
                (f"unoccupied_heat_{zone}", "TUnoHeaSet"),
                (f"unoccupied_cool_{zone}", "TUnoCooSet"),
                (f"window_closed_{zone}", "u1Win"),
                (f"optimal_cooldown_{zone}_predictor.seconds", "cooDowTim"),
                (f"optimal_warmup_{zone}_predictor.seconds", "warUpTim"),
            ):
                edges.append(wire(source, f"{name}.{target}"))
            status_outputs["warmup"].append(f"{name}.yOccHeaHig")
            status_outputs["cooldown"].append(f"{name}.yHigOccCoo")
            for selector, output in (
                ("cold_window", "yUnoHeaHig"),
                ("hot_window", "yHigUnoCoo"),
                ("warm_end_window", "yEndSetBac"),
                ("cool_end_window", "yEndSetUp"),
            ):
                old_source = f"{selector}_{zone}.y"
                for edge in edges:
                    if edge["from"] == old_source:
                        edge["from"] = f"{name}.{output}"
            for mode, output in (("warmup", "yWarTim"), ("cooldown", "yCooTim")):
                predictor = f"optimal_{mode}_{zone}_predictor.seconds"
                for edge in edges:
                    if edge["from"] == predictor and not edge["to"].startswith(f"{name}."):
                        edge["from"] = f"{name}.{output}"

        for mode in ("warmup", "cooldown"):
            previous = status_outputs[mode][0]
            for zone in range(2, 6):
                aggregator = f"{mode}_status_any_{zone}"
                blocks.append(binary(aggregator, "Or", "Boolean", "Boolean",
                                     "An occupied-setpoint temperature deviation in any closed-window zone requests recovery."))
                edges.extend((wire(previous, f"{aggregator}.u1"),
                              wire(status_outputs[mode][zone - 1], f"{aggregator}.u2")))
                previous = f"{aggregator}.y"
            gate = f"{mode}_status_gate"
            blocks.append(binary(gate, "And", "Boolean", "Boolean",
                                 "Require the OBC zone-status temperature condition before group optimal start."))
            edges[:] = [edge for edge in edges if edge["to"] != f"{mode}_candidate.u1"]
            edges.extend((wire(f"{mode}_start.y", f"{gate}.u1"),
                          wire(previous, f"{gate}.u2"),
                          wire(f"{gate}.y", f"{mode}_candidate.u1")))
        obsolete = {"setback_end_margin", "setup_end_margin"}
        for zone in range(1, 6):
            for stem in ("cold", "hot", "warm_end", "cool_end", "warm_end_target",
                         "cool_end_target", "cold_window", "hot_window",
                         "warm_end_window", "cool_end_window"):
                obsolete.add(f"{stem}_{zone}")
        blocks[:] = [block for block in blocks if block["name"] not in obsolete]
        edges[:] = [edge for edge in edges
                    if edge["from"].split(".")[0] not in obsolete
                    and edge["to"].split(".")[0] not in obsolete]
        group["notes"] += (
            " Each zone now uses the editable OBC ZoneStatus temperature hysteresis, "
            "window-gated recovery time, and setback/setup status. Former duplicate "
            "comparators and window gates have been removed."
        )
