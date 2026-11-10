"""Make the former economizer cooling-demand gate an editable teaching choice."""


def add_optional_demand_gate(graph, port):
    for index in (1, 2):
        ahu = next(block for block in graph["blocks"] if block["name"] == f"ahu_{index}")
        ahu["blocks"].extend([
            {"name": "economizer_use_teaching_demand_gate",
             "block_type": "Buildings.Controls.OBC.CDL.Logical.Sources.Constant",
             "category": "elementary", "inputs": [], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "k", "type": "Boolean", "value": False}],
             "notes": "Optional teaching gate requiring return air above the supply target. Off follows the high-limit, fan-proof, and freeze enable conditions."},
            {"name": "economizer_demand_gate_bypass",
             "block_type": "Buildings.Controls.OBC.CDL.Logical.Sources.Constant",
             "category": "elementary", "inputs": [], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "k", "type": "Boolean", "value": True}],
             "notes": "Permit the economizer when its high limit, fan, and freeze conditions allow it; the PI still selects the actual outdoor command."},
            {"name": "economizer_demand_gate_select",
             "block_type": "Buildings.Controls.OBC.CDL.Logical.Switch",
             "category": "elementary",
             "inputs": [port("u1", "Boolean"), port("u2", "Boolean"), port("u3", "Boolean")],
             "outputs": [port("y", "Boolean")],
             "notes": "Select the former return-above-supply teaching gate or bypass it in the economizer enable path."},
        ])
        old = ("return_above_supply_target.y", "free_cooling_useful.u2")
        matches = [edge for edge in ahu["connections"] if (edge["from"], edge["to"]) == old]
        if len(matches) != 1:
            raise ValueError(f"Missing unique economizer demand gate in AHU {index}")
        ahu["connections"].remove(matches[0])
        ahu["connections"].extend({"from": source, "to": target} for source, target in [
            ("return_above_supply_target.y", "economizer_demand_gate_select.u1"),
            ("economizer_use_teaching_demand_gate.y", "economizer_demand_gate_select.u2"),
            ("economizer_demand_gate_bypass.y", "economizer_demand_gate_select.u3"),
            ("economizer_demand_gate_select.y", "free_cooling_useful.u2"),
        ])
        ahu["notes"] += " A selectable teaching gate can require return air above supply target; it is bypassed by default."
    graph["notes"] += " Economizer enable does not require a separate cooling-demand condition by default."
