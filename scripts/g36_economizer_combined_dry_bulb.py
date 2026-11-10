"""Selectable combined fixed and differential dry-bulb high limit in CDL."""


def add_combined_dry_bulb_high_limit(graph, port, binary):
    for index in (1, 2):
        ahu = next(block for block in graph["blocks"] if block["name"] == f"ahu_{index}")
        ahu["blocks"].extend([
            {"name": "economizer_use_combined_dry_bulb",
             "block_type": "Buildings.Controls.OBC.CDL.Logical.Sources.Constant",
             "category": "elementary", "inputs": [], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "k", "type": "Boolean", "value": False}],
             "notes": "Select the combined fixed and differential dry-bulb device. Keep false for the existing device choice."},
            binary("economizer_combined_dry_bulb_trip", "Or", "Boolean", "Boolean",
                   "Trip the combined high limit when either fixed or differential limit is exceeded."),
            {"name": "economizer_dry_bulb_device_select",
             "block_type": "Buildings.Controls.OBC.CDL.Logical.Switch",
             "category": "elementary",
             "inputs": [port("u1", "Boolean"), port("u2", "Boolean"), port("u3", "Boolean")],
             "outputs": [port("y", "Boolean")],
             "notes": "Choose the combined device or the existing fixed/differential dry-bulb choice before the hold."},
        ])
        old = ("economizer_dry_bulb_high_limit_select.y", "economizer_high_limit_hold.u")
        matches = [edge for edge in ahu["connections"]
                   if (edge["from"], edge["to"]) == old]
        if len(matches) != 1:
            raise ValueError(f"Missing unique high-limit device wire in AHU {index}")
        ahu["connections"].remove(matches[0])
        ahu["connections"].extend({"from": source, "to": target} for source, target in [
            ("economizer_fixed_dry_bulb_hysteresis.y", "economizer_combined_dry_bulb_trip.u1"),
            ("economizer_high_limit_hysteresis.y", "economizer_combined_dry_bulb_trip.u2"),
            ("economizer_combined_dry_bulb_trip.y", "economizer_dry_bulb_device_select.u1"),
            ("economizer_use_combined_dry_bulb.y", "economizer_dry_bulb_device_select.u2"),
            ("economizer_dry_bulb_high_limit_select.y", "economizer_dry_bulb_device_select.u3"),
            ("economizer_dry_bulb_device_select.y", "economizer_high_limit_hold.u"),
        ])
        ahu["notes"] += " An editable CDL switch also offers the combined fixed and differential dry-bulb high-limit device."
    graph["notes"] += " Economizer combined dry-bulb high limits are editable CDL choices."
