"""Add OBC AHU plant requests separately from hot/chilled reset requests."""


def add_ahu_plant_requests(graph, port):
    plant = next(block for block in graph["blocks"] if block["name"] == "plant")

    def edge(source, target):
        return {"from": source, "to": target}

    def integer_add(name):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Integers.Add",
                "category": "elementary", "inputs": [port("u1", "Integer", "count"),
                                                port("u2", "Integer", "count")],
                "outputs": [port("y", "Integer", "count")]}

    def boolean_or(name):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Logical.Or",
                "category": "elementary", "inputs": [port("u1", "Boolean"), port("u2", "Boolean")],
                "outputs": [port("y", "Boolean")]}

    for ahu in (1, 2):
        prefix = f"ahu_{ahu}"
        for water, valve in (("hot", "heating_valve"), ("chilled", "cooling_valve")):
            stem = f"{prefix}_{water}_plant"
            plant["blocks"].extend([
                {"name": f"{stem}_valve_hysteresis", "block_type": "Buildings.Controls.OBC.CDL.Reals.Hysteresis",
                 "category": "elementary", "inputs": [port("u", unit="fraction (0–1)")],
                 "outputs": [port("y", "Boolean")],
                 "parameters": [{"name": "uLow", "value": 0.10}, {"name": "uHigh", "value": 0.95}],
                 "notes": "OBC AHU plant request begins above 95% coil command and holds until below 10%."},
                {"name": f"{stem}_request_count", "block_type": "Buildings.Controls.OBC.CDL.Conversions.BooleanToInteger",
                 "category": "elementary", "inputs": [port("u", "Boolean")],
                 "outputs": [port("y", "Integer", "count")],
                 "notes": "Separate plant enable request from the supply-water reset tier."},
            ])
            plant["connections"].extend([
                edge(f"{prefix}_{valve}", f"{stem}_valve_hysteresis.u"),
                edge(f"{stem}_valve_hysteresis.y", f"{stem}_request_count.u"),
                edge(f"{stem}_request_count.y", f"{stem}_request"),
            ])
            plant["outputs"].append(port(f"{stem}_request", "Integer", "count",
                                          f"AHU {ahu} {water.title()} Water Plant Request", "Point"))
            graph["outputs"].append(port(f"{stem}_request", "Integer", "count",
                                          f"AHU {ahu} {water.title()} Water Plant Request", "Point"))
            graph["connections"].append(edge(f"plant.{stem}_request", f"{stem}_request"))

            # OBC TrueDelay has immediate output for an initially true input.
            # The old teaching proxy forced a startup delay even in that case.
            for level in ("two", "three"):
                delay = next(block for block in plant["blocks"]
                             if block["name"] == f"{prefix}_{'hot_' if water == 'hot' else ''}{level}_request_delay")
                next(parameter for parameter in delay["parameters"] if parameter["name"] == "delayOnInit")["value"] = False

    for water in ("hot", "chilled"):
        sum_name = f"ahu_{water}_plant_request_count"
        active = f"ahu_{water}_plant_request_active"
        enable = f"ahu_{water}_plant_request_or_load"
        plant["blocks"].extend([
            integer_add(sum_name),
            {"name": active, "block_type": "Buildings.Controls.OBC.CDL.Integers.GreaterThreshold",
             "category": "elementary", "inputs": [port("u", "Integer", "count")],
             "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "t", "type": "Integer", "value": 0}]},
            boolean_or(enable),
        ])
        existing = "hot_plant_request_or_heat.y" if water == "hot" else "chilled_water_pump_needed.y"
        destination = f"freeze_{'hot' if water == 'hot' else 'chilled'}_water_pump_enable.u1"
        plant["connections"] = [item for item in plant["connections"]
                                if (item["from"], item["to"]) != (existing, destination)]
        plant["connections"].extend([
            edge(f"ahu_1_{water}_plant_request_count.y", f"{sum_name}.u1"),
            edge(f"ahu_2_{water}_plant_request_count.y", f"{sum_name}.u2"),
            edge(f"{sum_name}.y", f"{active}.u"),
            edge(f"{active}.y", f"{enable}.u1"),
            edge(existing, f"{enable}.u2"),
            edge(f"{enable}.y", destination),
        ])
        output = f"{sum_name}_total"
        plant["outputs"].append(port(output, "Integer", "count", "AHU Plant Requests", "Point"))
        plant["connections"].append(edge(f"{sum_name}.y", output))
        graph["outputs"].append(port(f"plant_{output}", "Integer", "count", "AHU Plant Requests", "Point"))
        graph["connections"].append(edge(f"plant.{output}", f"plant_{output}"))

    plant["notes"] += " AHU plant requests now use separate OBC 95% start / 10% clear coil-valve latches; temperature reset tiers remain distinct."
