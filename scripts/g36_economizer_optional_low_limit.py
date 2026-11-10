"""Keep the former teaching low-temperature economizer guard selectable in CDL."""


def add_optional_low_limit(graph, port):
    for index in (1, 2):
        ahu = next(block for block in graph["blocks"] if block["name"] == f"ahu_{index}")
        ahu["blocks"].extend([
            {"name": "economizer_use_teaching_low_limit",
             "block_type": "Buildings.Controls.OBC.CDL.Logical.Sources.Constant",
             "category": "elementary", "inputs": [], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "k", "type": "Boolean", "value": False}],
             "notes": "Optional teaching low-temperature guard. False uses the high-limit and freeze sequence without this extra cutoff."},
            {"name": "economizer_low_limit_bypass",
             "block_type": "Buildings.Controls.OBC.CDL.Logical.Sources.Constant",
             "category": "elementary", "inputs": [], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "k", "type": "Boolean", "value": True}],
             "notes": "Permit free cooling below the optional teaching cutoff while safety gates remain active."},
            {"name": "economizer_low_limit_select",
             "block_type": "Buildings.Controls.OBC.CDL.Logical.Switch",
             "category": "elementary",
             "inputs": [port("u1", "Boolean"), port("u2", "Boolean"), port("u3", "Boolean")],
             "outputs": [port("y", "Boolean")],
             "notes": "Choose the editable teaching cutoff or bypass it before fan and freeze qualification."},
        ])
        old = ("outdoor_above_low_limit.y", "economizer_temperature_ok.u1")
        matches = [edge for edge in ahu["connections"]
                   if (edge["from"], edge["to"]) == old]
        if len(matches) != 1:
            raise ValueError(f"Missing unique economizer low-limit wire in AHU {index}")
        ahu["connections"].remove(matches[0])
        ahu["connections"].extend({"from": source, "to": target} for source, target in [
            ("outdoor_above_low_limit.y", "economizer_low_limit_select.u1"),
            ("economizer_use_teaching_low_limit.y", "economizer_low_limit_select.u2"),
            ("economizer_low_limit_bypass.y", "economizer_low_limit_select.u3"),
            ("economizer_low_limit_select.y", "economizer_temperature_ok.u1"),
        ])
        # LimPID otherwise integrates while the economizer output is deselected.
        # Reset on the rising enable edge, using the CDL PIDWithReset block already
        # supported by both Rust executors and used by the terminal controllers.
        pi = next(block for block in ahu["blocks"] if block["name"] == "economizer_mixed_air_pi")
        pi["block_type"] = "PIDWithReset"
        pi["inputs"].append(port("trigger", "Boolean"))
        pi["parameters"].extend([
            {"name": "r", "value": 1.0},
            {"name": "Ni", "value": 0.9},
            {"name": "y_reset", "value": 0.0},
            {"name": "xi_start", "value": 0.0},
        ])
        pi["notes"] += " Reset to a closed economizer command whenever CDL enables free cooling."
        ahu["connections"].append({"from": "economizer_enabled.y", "to": "economizer_mixed_air_pi.trigger"})
        ahu["notes"] += " The former 12 °C teaching low-temperature guard is optional CDL and is off by default; freeze protection still applies."
    graph["notes"] += " The sample does not impose the former teaching low-temperature economizer cutoff by default."
