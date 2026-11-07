"""Editable fixed or differential dry-bulb economizer high-limit device."""


def add_fixed_dry_bulb_high_limit(graph, port, constant, binary):
    for index in (1, 2):
        ahu = next(block for block in graph["blocks"] if block["name"] == f"ahu_{index}")
        ahu["blocks"].extend([
            constant("economizer_fixed_dry_bulb_cutoff", 24.0, "°C",
                     "Editable fixed outdoor dry-bulb high limit for climates using this device."),
            binary("economizer_fixed_dry_bulb_error", "Subtract", "Real", "Real",
                   "Outdoor temperature minus the fixed high-limit cutoff."),
            {"name": "economizer_fixed_dry_bulb_hysteresis", "block_type": "Hysteresis",
             "category": "elementary", "inputs": [port("u", unit="K")],
             "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "uLow", "value": -1.0, "unit": "K"},
                            {"name": "uHigh", "value": 0.0, "unit": "K"}],
             "notes": "Disable at the fixed cutoff; re-enable one kelvin below it."},
            {"name": "economizer_use_fixed_dry_bulb", "block_type": "Buildings.Controls.OBC.CDL.Logical.Sources.Constant",
             "category": "elementary", "inputs": [], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "k", "type": "Boolean", "value": False}],
             "notes": "Set true to select the fixed dry-bulb high-limit device; false uses differential dry bulb."},
            {"name": "economizer_dry_bulb_high_limit_select", "block_type": "Buildings.Controls.OBC.CDL.Logical.Switch",
             "category": "elementary",
             "inputs": [port("u1", "Boolean"), port("u2", "Boolean"), port("u3", "Boolean")],
             "outputs": [port("y", "Boolean")],
             "notes": "Choose the editable fixed or differential high-limit state before the ten-minute hold."},
        ])
        replaced = ("economizer_high_limit_hysteresis.y", "economizer_high_limit_hold.u")
        matches = [edge for edge in ahu["connections"]
                   if (edge["from"], edge["to"]) == replaced]
        if len(matches) != 1:
            raise ValueError(f"Missing unique differential high-limit wire in AHU {index}")
        ahu["connections"].remove(matches[0])
        ahu["connections"].extend({"from": source, "to": target} for source, target in [
            ("outdoor_c", "economizer_fixed_dry_bulb_error.u1"),
            ("economizer_fixed_dry_bulb_cutoff.y", "economizer_fixed_dry_bulb_error.u2"),
            ("economizer_fixed_dry_bulb_error.y", "economizer_fixed_dry_bulb_hysteresis.u"),
            ("economizer_fixed_dry_bulb_hysteresis.y", "economizer_dry_bulb_high_limit_select.u1"),
            ("economizer_use_fixed_dry_bulb.y", "economizer_dry_bulb_high_limit_select.u2"),
            ("economizer_high_limit_hysteresis.y", "economizer_dry_bulb_high_limit_select.u3"),
            ("economizer_dry_bulb_high_limit_select.y", "economizer_high_limit_hold.u"),
        ])
        ahu["notes"] += " A CDL switch selects fixed or differential dry-bulb economizer high limit."
    graph["notes"] += " Each AHU has an editable fixed versus differential dry-bulb high-limit choice."
