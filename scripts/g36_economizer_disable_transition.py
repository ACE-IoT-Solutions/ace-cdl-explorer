"""OBC-style economizer disable damper transition in editable CDL."""


def add_economizer_disable_transition(graph, port, binary, switch):
    for index in (1, 2):
        ahu = next(block for block in graph["blocks"] if block["name"] == f"ahu_{index}")
        ahu["blocks"].extend([
            {"name": "economizer_disable", "block_type": "Not", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "notes": "Raw economizer-disable request after high-limit, fan-proof, and freeze checks."},
            {"name": "economizer_outdoor_close_delay", "block_type": "TrueDelay", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "delayTime", "value": 15.0, "unit": "s"},
                            {"name": "delayOnInit", "type": "Boolean", "value": False}],
             "notes": "Wait 15 seconds before returning the outdoor damper to its minimum-air path after economizer disable."},
            {"name": "economizer_outdoor_transition_pending", "block_type": "Not", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "notes": "Retain the modulating outdoor-air path during the short disable transition."},
            binary("economizer_outdoor_transition_safe", "And", "Boolean", "Boolean",
                   "Fan proof and freeze state override the outdoor-air transition immediately."),
            {"name": "economizer_return_release_delay", "block_type": "TrueDelay", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "delayTime", "value": 180.0, "unit": "s"},
                            {"name": "delayOnInit", "type": "Boolean", "value": False}],
             "notes": "Keep the return damper at its physical maximum for 180 seconds after economizer disable."},
            {"name": "economizer_return_transition_pending", "block_type": "Not", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "notes": "The return-damper full-open interval has not elapsed."},
            binary("economizer_return_transition_active", "And", "Boolean", "Boolean",
                   "The economizer is disabled and the return-damper release delay remains active."),
            switch("economizer_return_transition_select", "fraction (0–1)",
                   "Fully open the return damper during disable transition, then release it to the normal minimum-air command."),
        ])
        replaced = {
            ("economizer_enabled.y", "outdoor_air_select.u3"),
            ("return_damper_physical_limit.y", "return_air_damper_position"),
        }
        found = {(edge["from"], edge["to"]) for edge in ahu["connections"]} & replaced
        if found != replaced:
            raise ValueError(f"Missing economizer disable transition wires in AHU {index}: {replaced-found}")
        ahu["connections"] = [edge for edge in ahu["connections"]
                              if (edge["from"], edge["to"]) not in replaced]
        ahu["connections"].extend({"from": source, "to": target} for source, target in [
            ("economizer_enabled.y", "economizer_disable.u"),
            ("economizer_disable.y", "economizer_outdoor_close_delay.u"),
            ("economizer_outdoor_close_delay.y", "economizer_outdoor_transition_pending.u"),
            ("economizer_outdoor_transition_pending.y", "economizer_outdoor_transition_safe.u1"),
            ("economizer_proven_and_clear.y", "economizer_outdoor_transition_safe.u2"),
            ("economizer_outdoor_transition_safe.y", "outdoor_air_select.u3"),
            ("economizer_disable.y", "economizer_return_release_delay.u"),
            ("economizer_return_release_delay.y", "economizer_return_transition_pending.u"),
            ("economizer_disable.y", "economizer_return_transition_active.u1"),
            ("economizer_return_transition_pending.y", "economizer_return_transition_active.u2"),
            ("return_damper_physical_max.y", "economizer_return_transition_select.u1"),
            ("return_damper_physical_limit.y", "economizer_return_transition_select.u2"),
            ("economizer_return_transition_active.y", "economizer_return_transition_select.u3"),
            ("economizer_return_transition_select.y", "return_air_damper_position"),
        ])
        ahu["notes"] += " CDL delays outdoor-air closure 15 seconds and holds the return damper fully open 180 seconds after economizer disable."
    graph["notes"] += " Economizer disable damper transitions follow editable CDL timers."
