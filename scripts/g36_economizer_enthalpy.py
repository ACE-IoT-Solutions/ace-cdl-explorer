"""Selectable fixed enthalpy plus fixed dry-bulb high limit in editable CDL."""


def add_enthalpy_high_limit(graph, port, constant, binary):
    point = "outdoor_air_enthalpy_kj_kg"
    graph["inputs"].append(port(point, unit="kJ/kg", label="Outside Air Enthalpy Sensor",
                                brick="Point", description="Raw specific enthalpy of outdoor moist air from the weather and humidity scenario."))
    if graph.get("brick_model"):
        graph["brick_model"]["entities"].append({
            "id": point, "class": "brick:Point", "label": "Outside air enthalpy",
            "connector": point,
        })
        graph["brick_model"]["relationships"].append({
            "subject": "building_01", "predicate": "brick:hasPoint", "object": point,
        })
    for index in (1, 2):
        ahu = next(block for block in graph["blocks"] if block["name"] == f"ahu_{index}")
        ahu["inputs"].append(port(point, unit="kJ/kg", label="Outside Air Enthalpy Sensor",
                                  brick="Point"))
        graph["connections"].append({"from": point, "to": f"ahu_{index}.{point}"})
        ahu["blocks"].extend([
            constant("economizer_enthalpy_cutoff", 66.0, "kJ/kg", "Outdoor air enthalpy high-limit cutoff."),
            binary("economizer_enthalpy_error", "Subtract", "Real", "Real",
                   "Measured outdoor enthalpy minus the editable high-limit cutoff."),
            {"name": "economizer_enthalpy_hysteresis", "block_type": "Hysteresis",
             "category": "elementary", "inputs": [port("u", unit="kJ/kg")],
             "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "uLow", "value": -1.0, "unit": "kJ/kg"},
                            {"name": "uHigh", "value": 0.0, "unit": "kJ/kg"}],
             "notes": "Trip above the enthalpy cutoff and clear one kJ/kg below it."},
            binary("economizer_enthalpy_and_dry_bulb_trip", "Or", "Boolean", "Boolean",
                   "Disable economizing when either fixed enthalpy or fixed dry bulb trips."),
            {"name": "economizer_use_enthalpy_device",
             "block_type": "Buildings.Controls.OBC.CDL.Logical.Sources.Constant",
             "category": "elementary", "inputs": [], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "k", "type": "Boolean", "value": False}],
             "notes": "Set true to select fixed enthalpy plus fixed dry bulb; false retains the prior dry-bulb choice."},
            {"name": "economizer_high_limit_device_select", "block_type": "Buildings.Controls.OBC.CDL.Logical.Switch",
             "category": "elementary",
             "inputs": [port("u1", "Boolean"), port("u2", "Boolean"), port("u3", "Boolean")],
             "outputs": [port("y", "Boolean")],
             "notes": "Select an enthalpy/dry-bulb or dry-bulb-only trip before qualification and hold."},
        ])
        ahu["connections"].extend({"from": source, "to": target} for source, target in [
            (point, "economizer_enthalpy_error.u1"),
            ("economizer_enthalpy_cutoff.y", "economizer_enthalpy_error.u2"),
            ("economizer_enthalpy_error.y", "economizer_enthalpy_hysteresis.u"),
            ("economizer_enthalpy_hysteresis.y", "economizer_enthalpy_and_dry_bulb_trip.u1"),
            ("economizer_fixed_dry_bulb_hysteresis.y", "economizer_enthalpy_and_dry_bulb_trip.u2"),
            ("economizer_enthalpy_and_dry_bulb_trip.y", "economizer_high_limit_device_select.u1"),
            ("economizer_use_enthalpy_device.y", "economizer_high_limit_device_select.u2"),
            ("economizer_dry_bulb_device_select.y", "economizer_high_limit_device_select.u3"),
            ("economizer_high_limit_device_select.y", "economizer_high_limit_hold.u"),
            ("economizer_high_limit_device_select.y", "economizer_high_limit_sustained.u"),
            ("economizer_high_limit_device_select.y", "economizer_high_limit_clear.u"),
        ])
        for target in ("economizer_high_limit_hold.u", "economizer_high_limit_sustained.u",
                       "economizer_high_limit_clear.u"):
            old = [edge for edge in ahu["connections"] if edge["to"] == target and
                   edge["from"] == "economizer_dry_bulb_device_select.y"]
            if len(old) != 1:
                raise ValueError(f"Missing unique dry-bulb high-limit wire to {target} in AHU {index}")
            ahu["connections"].remove(old[0])
        ahu["notes"] += " An editable CDL device can also select fixed enthalpy plus fixed dry bulb using raw outdoor moist-air enthalpy."
    graph["notes"] += " Both AHUs offer an editable enthalpy/dry-bulb economizer high-limit choice."
