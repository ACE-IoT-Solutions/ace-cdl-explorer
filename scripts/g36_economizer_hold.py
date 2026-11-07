"""G36 economizer high-limit state hold with immediate safety overrides."""


def add_economizer_high_limit_hold(graph, port):
    for index in (1, 2):
        ahu = next(block for block in graph["blocks"] if block["name"] == f"ahu_{index}")
        ahu["blocks"].append({
            "name": "economizer_high_limit_hold",
            "block_type": "Buildings.Controls.OBC.CDL.Logical.TrueFalseHold",
            "category": "elementary",
            "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
            "parameters": [
                {"name": "trueHoldDuration", "value": 600.0, "unit": "s"},
                {"name": "falseHoldDuration", "value": 600.0, "unit": "s"},
            ],
            "notes": "Hold high-limit disabled and enabled states for at least ten minutes after a transition. Fan loss and freeze protection bypass this hold for immediate shutdown.",
        })
        for edge in ahu["connections"]:
            if edge["from"] == "economizer_high_limit_hysteresis.y" and edge["to"] == "economizer_high_limit_ok.u":
                edge["to"] = "economizer_high_limit_hold.u"
                break
        else:
            raise ValueError("Missing G36 economizer high-limit connection")
        ahu["connections"].append({"from": "economizer_high_limit_hold.y",
                                   "to": "economizer_high_limit_ok.u"})
        ahu["notes"] += " A CDL ten-minute high-limit state hold reduces economizer mode cycling; fan and freeze safety gates remain immediate."
    graph["notes"] += " Economizer high-limit state holds are editable CDL, with separate immediate fan and freeze gates."
