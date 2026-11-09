"""Replace teaching reheat requests with the pinned OBC SystemRequests hot-water tiers."""


def add_terminal_hot_request_levels(graph, port):
    def edge(source, target):
        return {"from": source, "to": target}

    def constant(name, value, kind="Real", unit=None):
        return {"name": name, "block_type": f"Buildings.Controls.OBC.CDL.{ 'Integers' if kind == 'Integer' else 'Reals' }.Sources.Constant",
                "category": "elementary", "inputs": [], "outputs": [port("y", kind, unit)],
                "parameters": [{"name": "k", "type": kind, "value": value, **({"unit": unit} if unit else {})}]}

    def binary(name, kind, unit=None):
        return {"name": name, "block_type": f"Buildings.Controls.OBC.CDL.Reals.{kind}",
                "category": "elementary", "inputs": [port("u1", "Real", unit), port("u2", "Real", unit)],
                "outputs": [port("y", "Boolean" if kind in ("Greater", "Less") else "Real",
                                 None if kind in ("Greater", "Less") else "K")]}

    def delay(name):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Logical.TrueDelay",
                "category": "elementary", "inputs": [port("u", "Boolean")],
                "outputs": [port("y", "Boolean")],
                "parameters": [{"name": "delayTime", "type": "Real", "value": 300.0, "unit": "s"},
                               {"name": "delayOnInit", "type": "Boolean", "value": False}]}

    def hysteresis(name, low):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Reals.Hysteresis",
                "category": "elementary", "inputs": [port("u", "Real", "fraction (0–1)")],
                "outputs": [port("y", "Boolean")],
                "parameters": [{"name": "uLow", "type": "Real", "value": low},
                               {"name": "uHigh", "type": "Real", "value": 0.95}]}

    def switch(name):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Integers.Switch",
                "category": "elementary", "inputs": [port("u1", "Integer", "count"), port("u2", "Boolean"),
                                                  port("u3", "Integer", "count")],
                "outputs": [port("y", "Integer", "count")]}

    def integer_add(name):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Integers.Add",
                "category": "elementary", "inputs": [port("u1", "Integer", "count"),
                                                  port("u2", "Integer", "count")],
                "outputs": [port("y", "Integer", "count")]}

    for index in range(1, 11):
        zone = next(block for block in graph["blocks"] if block["name"] == f"zone_{index:02}")
        if any(block["name"] == "hot_reset_three_requests" for block in zone["blocks"]):
            continue
        zone["blocks"].extend([
            binary("hot_reset_discharge_error", "Subtract", "°C"),
            constant("hot_reset_two_threshold", 8.0, unit="K"),
            constant("hot_reset_three_threshold", 17.0, unit="K"),
            binary("hot_reset_two_deficit", "Greater", "K"),
            binary("hot_reset_three_deficit", "Greater", "K"),
            delay("hot_reset_two_delay"), delay("hot_reset_three_delay"),
            hysteresis("hot_reset_valve_hysteresis", 0.85),
            hysteresis("hot_plant_valve_hysteresis", 0.10),
            *[constant(f"hot_reset_level_{level}", level, "Integer", "count") for level in range(4)],
            switch("hot_reset_one_request"), switch("hot_reset_two_requests"),
            switch("hot_reset_three_requests"),
            {"name": "hot_plant_request_integer", "block_type": "Buildings.Controls.OBC.CDL.Conversions.BooleanToInteger",
             "category": "elementary", "inputs": [port("u", "Boolean")],
             "outputs": [port("y", "Integer", "count")]},
        ])
        zone["outputs"].extend([
            port("hot_water_reset_request", "Integer", "count", "Hot Water Reset Request", "Point"),
            port("hot_water_plant_request", "Integer", "count", "Hot Water Plant Request", "Point"),
        ])
        zone["connections"].extend(edge(source, target) for source, target in [
            ("discharge_target.y", "hot_reset_discharge_error.u1"),
            ("discharge_temp", "hot_reset_discharge_error.u2"),
            ("hot_reset_discharge_error.y", "hot_reset_two_deficit.u1"),
            ("hot_reset_two_threshold.y", "hot_reset_two_deficit.u2"),
            ("hot_reset_discharge_error.y", "hot_reset_three_deficit.u1"),
            ("hot_reset_three_threshold.y", "hot_reset_three_deficit.u2"),
            ("hot_reset_two_deficit.y", "hot_reset_two_delay.u"),
            ("hot_reset_three_deficit.y", "hot_reset_three_delay.u"),
            ("reheat_capped.y", "hot_reset_valve_hysteresis.u"),
            ("reheat_capped.y", "hot_plant_valve_hysteresis.u"),
            ("hot_reset_level_1.y", "hot_reset_one_request.u1"),
            ("hot_reset_valve_hysteresis.y", "hot_reset_one_request.u2"),
            ("hot_reset_level_0.y", "hot_reset_one_request.u3"),
            ("hot_reset_level_2.y", "hot_reset_two_requests.u1"),
            ("hot_reset_two_delay.y", "hot_reset_two_requests.u2"),
            ("hot_reset_one_request.y", "hot_reset_two_requests.u3"),
            ("hot_reset_level_3.y", "hot_reset_three_requests.u1"),
            ("hot_reset_three_delay.y", "hot_reset_three_requests.u2"),
            ("hot_reset_two_requests.y", "hot_reset_three_requests.u3"),
            ("hot_reset_three_requests.y", "hot_water_reset_request"),
            ("hot_plant_valve_hysteresis.y", "hot_plant_request_integer.u"),
            ("hot_plant_request_integer.y", "hot_water_plant_request"),
        ])
        zone["notes"] += " Pinned OBC SystemRequests hot-water tiers issue 3/2 after five minutes more than 17/8 K below the discharge target, or 1 while the reheat valve is above 95% (clear below 85%). A separate plant request holds until the valve falls below 10%."
        for suffix, output in (("hot_water_reset_request", "Hot Water Reset Request"),
                               ("hot_water_plant_request", "Hot Water Plant Request")):
            name = f"zone_{index:02}_{suffix}"
            graph["outputs"].append(port(name, "Integer", "count", f"Zone {index} {output}", "Point"))
            graph["connections"].append(edge(f"zone_{index:02}.{suffix}", name))

    plant = next(block for block in graph["blocks"] if block["name"] == "plant")
    old = {"terminal_heat_demand_threshold", "hot_reset_all_request_count"}
    old.update(block["name"] for block in plant["blocks"] if block["name"].startswith("terminal_hot_sum_")
               or (block["name"].startswith("zone_") and "_hot_" in block["name"]))
    plant["blocks"] = [block for block in plant["blocks"] if block["name"] not in old]
    plant["connections"] = [item for item in plant["connections"]
                            if item["from"].split(".")[0] not in old and item["to"].split(".")[0] not in old]
    old_inputs = {f"zone_{index:02}_{suffix}" for index in range(1, 11)
                  for suffix in ("reheat_valve", "heating_demand")}
    plant["inputs"] = [item for item in plant["inputs"] if item["name"] not in old_inputs]
    graph["connections"] = [item for item in graph["connections"]
                            if item["to"] not in {f"plant.{name}" for name in old_inputs}]
    for index in range(1, 11):
        for suffix in ("hot_water_reset_request", "hot_water_plant_request"):
            name = f"zone_{index:02}_{suffix}"
            plant["inputs"].append(port(name, "Integer", "count", f"Zone {index} {suffix.replace('_', ' ').title()}", "Point"))
            graph["connections"].append(edge(f"zone_{index:02}.{suffix}", f"plant.{name}"))

    def sum_tree(suffix, stem):
        requests = [f"zone_{index:02}_{suffix}" for index in range(1, 11)]
        level = 0
        while len(requests) > 1:
            following = []
            for pair in range(0, len(requests), 2):
                if pair + 1 == len(requests):
                    following.append(requests[pair])
                    continue
                name = f"{stem}_{level}_{pair // 2}"
                plant["blocks"].append(integer_add(name))
                plant["connections"].extend([edge(requests[pair], f"{name}.u1"),
                                             edge(requests[pair + 1], f"{name}.u2")])
                following.append(f"{name}.y")
            requests = following
            level += 1
        return requests[0]

    reset_sum = sum_tree("hot_water_reset_request", "terminal_hot_reset_sum")
    plant_sum = sum_tree("hot_water_plant_request", "terminal_hot_plant_sum")
    plant["blocks"].extend([
        integer_add("hot_reset_all_request_count"),
        {"name": "terminal_hot_plant_request_active", "block_type": "Buildings.Controls.OBC.CDL.Integers.GreaterThreshold",
         "category": "elementary", "inputs": [port("u", "Integer", "count")],
         "outputs": [port("y", "Boolean")], "parameters": [{"name": "t", "type": "Integer", "value": 0}]},
        {"name": "hot_plant_request_or_heat", "block_type": "Buildings.Controls.OBC.CDL.Logical.Or",
         "category": "elementary", "inputs": [port("u1", "Boolean"), port("u2", "Boolean")],
         "outputs": [port("y", "Boolean")]},
    ])
    plant["connections"] = [item for item in plant["connections"]
                            if (item["from"], item["to"]) !=
                            ("hot_water_pump_needed.y", "freeze_hot_water_pump_enable.u1")
                            and (item["to"] != "hot_reset_request_real.u" or
                                 item["from"] == "hot_reset_with_freeze_requests.y")]
    plant["connections"].extend(edge(source, target) for source, target in [
        (reset_sum, "terminal_hot_water_request_count"),
        ("hot_reset_request_count.y", "hot_reset_all_request_count.u1"),
        (reset_sum, "hot_reset_all_request_count.u2"),
        ("hot_reset_all_request_count.y", "hot_reset_with_freeze_requests.u1"),
        (plant_sum, "terminal_hot_water_plant_request_count"),
        (plant_sum, "terminal_hot_plant_request_active.u"),
        ("terminal_hot_plant_request_active.y", "hot_plant_request_or_heat.u1"),
        ("hot_water_pump_needed.y", "hot_plant_request_or_heat.u2"),
        ("hot_plant_request_or_heat.y", "freeze_hot_water_pump_enable.u1"),
    ])
    plant["outputs"].append(port("terminal_hot_water_plant_request_count", "Integer", "count",
                                  "Terminal Hot Water Plant Requests", "Point"))
    graph["outputs"].append(port("plant_terminal_hot_water_plant_request_count", "Integer", "count",
                                  "Terminal Hot Water Plant Requests", "Point"))
    graph["connections"].append(edge("plant.terminal_hot_water_plant_request_count",
                                      "plant_terminal_hot_water_plant_request_count"))
    plant["notes"] += " Terminal hot-water reset requests now follow OBC discharge-temperature and valve tiers. Separate valve-based plant requests enable the hot-water loop."
