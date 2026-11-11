"""Optional differential enthalpy plus fixed dry-bulb high limit in CDL."""


def add_differential_enthalpy_high_limit(graph, port, binary):
    for index in (1, 2):
        point = f"ahu_{index}_return_air_enthalpy_kj_kg"
        graph["inputs"].append(port(point, unit="kJ/kg", label="Return Air Enthalpy",
                                    brick="Point", description="Raw return-air specific enthalpy from the physical AHU return temperature and assumed return humidity."))
        graph["connections"].append({"from": point, "to": f"ahu_{index}.return_air_enthalpy_kj_kg"})
        ahu = next(block for block in graph["blocks"] if block["name"] == f"ahu_{index}")
        ahu["inputs"].append(port("return_air_enthalpy_kj_kg", unit="kJ/kg",
                                  label="Return Air Enthalpy", brick="Point"))
        ahu["blocks"].extend([
            binary("economizer_differential_enthalpy_error", "Subtract", "Real", "Real",
                   "Outdoor minus return-air specific enthalpy; both inputs are raw physical quantities."),
            {"name": "economizer_differential_enthalpy_hysteresis", "block_type": "Hysteresis",
             "category": "elementary", "inputs": [port("u", unit="kJ/kg")],
             "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "uLow", "value": -1.0, "unit": "kJ/kg"},
                            {"name": "uHigh", "value": 0.0, "unit": "kJ/kg"}],
             "notes": "Trip when outdoor enthalpy exceeds return enthalpy; clear 1 kJ/kg below it."},
            binary("economizer_differential_enthalpy_and_dry_bulb_trip", "Or", "Boolean", "Boolean",
                   "Trip when either outdoor enthalpy exceeds return enthalpy or the fixed dry-bulb limit trips."),
            {"name": "economizer_use_differential_enthalpy_device",
             "block_type": "Buildings.Controls.OBC.CDL.Logical.Sources.Constant",
             "category": "elementary", "inputs": [], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "k", "type": "Boolean", "value": False}],
             "notes": "Select differential enthalpy plus fixed dry bulb. This choice takes priority over the fixed-enthalpy choice."},
            {"name": "economizer_differential_enthalpy_device_select",
             "block_type": "Buildings.Controls.OBC.CDL.Logical.Switch", "category": "elementary",
             "inputs": [port("u1", "Boolean"), port("u2", "Boolean"), port("u3", "Boolean")],
             "outputs": [port("y", "Boolean")],
             "notes": "Select differential enthalpy or the existing CDL high-limit device before qualification."},
        ])
        ahu["connections"].extend({"from": source, "to": target} for source, target in [
            ("outdoor_air_enthalpy_kj_kg", "economizer_differential_enthalpy_error.u1"),
            ("return_air_enthalpy_kj_kg", "economizer_differential_enthalpy_error.u2"),
            ("economizer_differential_enthalpy_error.y", "economizer_differential_enthalpy_hysteresis.u"),
            ("economizer_differential_enthalpy_hysteresis.y", "economizer_differential_enthalpy_and_dry_bulb_trip.u1"),
            ("economizer_fixed_dry_bulb_hysteresis.y", "economizer_differential_enthalpy_and_dry_bulb_trip.u2"),
            ("economizer_differential_enthalpy_and_dry_bulb_trip.y", "economizer_differential_enthalpy_device_select.u1"),
            ("economizer_use_differential_enthalpy_device.y", "economizer_differential_enthalpy_device_select.u2"),
            ("economizer_high_limit_device_select.y", "economizer_differential_enthalpy_device_select.u3"),
            ("economizer_differential_enthalpy_device_select.y", "economizer_high_limit_hold.u"),
            ("economizer_differential_enthalpy_device_select.y", "economizer_high_limit_sustained.u"),
            ("economizer_differential_enthalpy_device_select.y", "economizer_high_limit_clear.u"),
        ])
        for target in ("economizer_high_limit_hold.u", "economizer_high_limit_sustained.u",
                       "economizer_high_limit_clear.u"):
            old = [edge for edge in ahu["connections"] if edge["to"] == target and
                   edge["from"] == "economizer_high_limit_device_select.y"]
            if len(old) != 1:
                raise ValueError(f"Missing unique enthalpy device wire to {target} in AHU {index}")
            ahu["connections"].remove(old[0])
        if graph.get("brick_model"):
            graph["brick_model"]["entities"].append({
                "id": point, "class": "brick:Point", "label": f"AHU {index} return air enthalpy",
                "connector": point,
            })
            graph["brick_model"]["relationships"].append({
                "subject": f"ahu_{index}_equipment", "predicate": "brick:hasPoint", "object": point,
            })
        ahu["notes"] += " An editable differential-enthalpy and fixed-dry-bulb high-limit device uses raw return-air enthalpy."
    graph["notes"] += " Both AHUs offer a CDL differential-enthalpy high-limit choice."
