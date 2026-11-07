"""Raw window status and editable CDL window handling for G36 teaching groups."""


def add_window_control(graph, port, constant, binary, switch):
    def wire(source, target):
        return {"from": source, "to": target}

    def boolean(name, kind, note):
        return binary(name, kind, "Boolean", "Boolean", note)

    for group_index in (1, 2):
        name = f"zone_group_{group_index}"
        group = next(block for block in graph["blocks"] if block["name"] == name)
        blocks, edges = group["blocks"], group["connections"]
        blocks.append(constant("window_zero_error", 0.0, "K", "Open windows do not contribute to optimal-start need."))
        blocks.append(constant("window_zero_temperature", 0.0, "°C", "Open windows do not contribute to group temperature averages."))
        for local in range(1, 6):
            zone_index = (group_index - 1) * 5 + local
            zone_name = f"zone_{zone_index:02}"
            sensor = f"{zone_name}_window_closed"
            graph["inputs"].append(port(sensor, "Boolean", "unitless", f"Zone {zone_index} Window Status",
                                        "Status", "Raw contact: true when closed, false when open."))
            graph["connections"].extend([wire(sensor, f"{name}.window_closed_{local}"),
                                          wire(sensor, f"{zone_name}.window_closed")])
            group["inputs"].append(port(f"window_closed_{local}", "Boolean"))
            blocks.append({"name": f"window_open_{local}", "block_type": "Not", "category": "elementary",
                           "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
                           "notes": "Open contact excludes this zone from ordinary group mode decisions."})
            edges.append(wire(f"window_closed_{local}", f"window_open_{local}.u"))
            for stem in ("warmup", "cooldown"):
                selected = f"{stem}_window_error_{local}"
                blocks.append(switch(selected, "K", "Use zero start need when this zone window is open."))
                source = f"{stem}_error_{local}.y"
                for edge in edges:
                    if edge["from"] == source and edge["to"].startswith(f"{stem}_maximum_"):
                        edge["from"] = f"{selected}.y"
                edges.extend([wire(source, f"{selected}.u1"),
                              wire("window_zero_error.y", f"{selected}.u2"),
                              wire(f"window_closed_{local}", f"{selected}.u3")])
            for stem, source in (("room", f"room_{local}"),
                                 ("heat", f"unoccupied_heat_{local}"),
                                 ("cool", f"unoccupied_cool_{local}")):
                selected = f"window_{stem}_{local}"
                blocks.append(switch(selected, "°C", "Exclude an open-window zone from the group average."))
                for edge in edges:
                    if edge["from"] == source and (edge["to"].startswith(f"sum_{stem}_") or
                                                   edge["to"].startswith("average_") and stem in ("room", "heat", "cool")):
                        edge["from"] = f"{selected}.y"
                edges.extend([wire(source, f"{selected}.u1"),
                              wire("window_zero_temperature.y", f"{selected}.u2"),
                              wire(f"window_closed_{local}", f"{selected}.u3")])
            # A window-open zone cannot start ordinary setback/setup and cannot hold
            # either latch after the remaining closed zones have recovered.
            for stem in ("cold", "hot"):
                selected = f"{stem}_window_{local}"
                blocks.append(boolean(selected, "And", "Require a closed window for an ordinary mode request."))
                source = f"{stem}_{local}.y"
                for edge in edges:
                    if edge["from"] == source and edge["to"].startswith(f"all_{stem}_"):
                        edge["from"] = f"{selected}.y"
                edges.extend([wire(source, f"{selected}.u1"),
                              wire(f"window_closed_{local}", f"{selected}.u2")])
            for stem, aggregate in (("warm_end", "all_warm"), ("cool_end", "all_cool")):
                selected = f"{stem}_window_{local}"
                blocks.append(boolean(selected, "Or", "Ignore an open-window zone when clearing an ordinary mode."))
                source = f"{stem}_{local}.y"
                for edge in edges:
                    if edge["from"] == source and edge["to"].startswith(f"{aggregate}_"):
                        edge["from"] = f"{selected}.y"
                edges.extend([wire(source, f"{selected}.u1"),
                              wire(f"window_open_{local}.y", f"{selected}.u2")])
            zone = next(block for block in graph["blocks"] if block["name"] == zone_name)
            zone["inputs"].append(port("window_closed", "Boolean", "unitless", "Window Closed", "Status"))
            zone["blocks"].extend([
                boolean("window_heat_permit", "Or", "Freeze setback keeps heating available with a window open."),
                switch("window_heat_target", "°C", "Disable ordinary heating while the window is open."),
                switch("window_cool_target", "°C", "Disable cooling while the window is open."),
            ])
            zone_edges = zone["connections"]
            for edge in zone_edges:
                if edge["from"] == "heat_setpoint_active.y":
                    edge["from"] = "window_heat_target.y"
                elif edge["from"] == "cool_setpoint_active.y":
                    edge["from"] = "window_cool_target.y"
            zone_edges.extend([wire("window_closed", "window_heat_permit.u1"),
                               wire("group_freeze_setback", "window_heat_permit.u2"),
                               wire("heat_setpoint_active.y", "window_heat_target.u1"),
                               wire("unoccupied_heat_off.y", "window_heat_target.u2"),
                               wire("window_heat_permit.y", "window_heat_target.u3"),
                               wire("cool_setpoint_active.y", "window_cool_target.u1"),
                               wire("unoccupied_cool_off.y", "window_cool_target.u2"),
                               wire("window_closed", "window_cool_target.u3")])
            zone["notes"] += " Open windows disable ordinary terminal heating and cooling; group freeze setback retains heating."
        group["notes"] += " Open-window zones are excluded from ordinary group start and average decisions; freeze protection still sees every zone."
    graph["notes"] += " Raw window contacts feed editable CDL group exclusion and terminal conditioning gates."
