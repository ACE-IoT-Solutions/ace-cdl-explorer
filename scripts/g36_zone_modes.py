"""Editable CDL teaching modes for the G36 sample's local VAV zones."""


def add_zone_conditioning_modes(graph, port, binary):
    def mode_constant(name, code, note):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
                "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "count")],
                "parameters": [{"name": "k", "type": "Integer", "value": code}], "notes": note}

    def mode_switch(name, note):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Integers.Switch",
                "category": "elementary",
                "inputs": [port("u1", "Integer", "count"), port("u2", "Boolean"),
                           port("u3", "Integer", "count")],
                "outputs": [port("y", "Integer", "count")], "notes": note}

    for index in range(1, 11):
        name = f"zone_{index:02}"
        zone = next(block for block in graph["blocks"] if block["name"] == name)
        zone["outputs"].append(port("conditioning_state", "Integer", "count",
                                    "Local Conditioning State", "Point",
                                    "Teaching codes: 0 unoccupied, 1 occupied, 2 sensor standby, 3 warm-up, 4 cooldown, 5 setup, 6 setback"))
        graph["outputs"].append(port(f"{name}_conditioning_state", "Integer", "count",
                                     f"Zone {index} Conditioning State", "Point"))
        graph["connections"].append({"from": f"{name}.conditioning_state",
                                      "to": f"{name}_conditioning_state"})
        blocks, edges = zone["blocks"], zone["connections"]
        blocks.extend([
            binary("warmup_error", "Subtract", "Real", "Real", "Occupied heating target minus measured room temperature."),
            binary("cooldown_error", "Subtract", "Real", "Real", "Measured room temperature minus occupied cooling target."),
            binary("setback_error", "Subtract", "Real", "Real", "Unoccupied heating target minus measured room temperature."),
            binary("setup_error", "Subtract", "Real", "Real", "Measured room temperature minus unoccupied cooling target."),
        ])
        for stem in ("warmup", "cooldown", "setback", "setup"):
            blocks.append({"name": f"{stem}_needed", "block_type": "Buildings.Controls.OBC.CDL.Reals.Hysteresis",
                           "category": "elementary", "inputs": [port("u", unit="K")],
                           "outputs": [port("y", "Boolean")],
                           "parameters": [{"name": "uLow", "value": -0.1, "unit": "K"},
                                          {"name": "uHigh", "value": 0.1, "unit": "K"}],
                           "notes": "Editable temperature deadband prevents mode chatter near the target."})
        blocks.extend([
            binary("warmup_mode", "And", "Boolean", "Boolean", "Warm-up only inside the schedule lookahead window."),
            binary("cooldown_mode", "And", "Boolean", "Boolean", "Cooldown only inside the schedule lookahead window."),
            binary("preconditioning_needed", "Or", "Boolean", "Boolean", "Restore occupied targets only when warm-up or cooldown is needed."),
            binary("occupied_setpoint_requested", "Or", "Boolean", "Boolean", "Use occupied setpoints for confirmed presence or thermal preconditioning."),
        ])
        for code, label in ((0, "unoccupied"), (1, "occupied"), (2, "standby"),
                            (3, "warmup"), (4, "cooldown"), (5, "setup"), (6, "setback")):
            blocks.append(mode_constant(f"mode_{label}", code, f"Local teaching mode code for {label}."))
        for selector in ("setback", "setup", "cooldown", "warmup", "occupied", "schedule"):
            blocks.append(mode_switch(f"mode_select_{selector}", f"Choose the {selector} mode from CDL conditions."))
        edges[:] = [edge for edge in edges if edge["to"] not in
                    {"conditioning_mode.u2", "heating_warmup_airflow.u3",
                     "heat_setpoint_standby.u3", "cool_setpoint_standby.u3"}]
        edges.extend({"from": source, "to": target} for source, target in [
            ("heat_setpoint.y", "warmup_error.u1"), ("room_temp", "warmup_error.u2"),
            ("room_temp", "cooldown_error.u1"), ("cool_setpoint.y", "cooldown_error.u2"),
            ("unoccupied_heat.y", "setback_error.u1"), ("room_temp", "setback_error.u2"),
            ("room_temp", "setup_error.u1"), ("unoccupied_cool.y", "setup_error.u2"),
            ("warmup_error.y", "warmup_needed.u"), ("cooldown_error.y", "cooldown_needed.u"),
            ("setback_error.y", "setback_needed.u"), ("setup_error.y", "setup_needed.u"),
            ("preconditioning_mode", "warmup_mode.u1"), ("warmup_needed.y", "warmup_mode.u2"),
            ("preconditioning_mode", "cooldown_mode.u1"), ("cooldown_needed.y", "cooldown_mode.u2"),
            ("warmup_mode.y", "preconditioning_needed.u1"),
            ("cooldown_mode.y", "preconditioning_needed.u2"),
            ("preconditioning_needed.y", "conditioning_mode.u2"),
            ("occupancy_latch.y", "occupied_setpoint_requested.u1"),
            ("preconditioning_needed.y", "occupied_setpoint_requested.u2"),
            ("occupied_setpoint_requested.y", "heat_setpoint_standby.u3"),
            ("occupied_setpoint_requested.y", "cool_setpoint_standby.u3"),
            ("warmup_mode.y", "heating_warmup_airflow.u3"),
            ("mode_setback.y", "mode_select_setback.u1"),
            ("setback_needed.y", "mode_select_setback.u2"),
            ("mode_unoccupied.y", "mode_select_setback.u3"),
            ("mode_setup.y", "mode_select_setup.u1"),
            ("setup_needed.y", "mode_select_setup.u2"),
            ("mode_select_setback.y", "mode_select_setup.u3"),
            ("mode_cooldown.y", "mode_select_cooldown.u1"),
            ("cooldown_mode.y", "mode_select_cooldown.u2"),
            ("mode_select_setup.y", "mode_select_cooldown.u3"),
            ("mode_warmup.y", "mode_select_warmup.u1"),
            ("warmup_mode.y", "mode_select_warmup.u2"),
            ("mode_select_cooldown.y", "mode_select_warmup.u3"),
            ("mode_occupied.y", "mode_select_occupied.u1"),
            ("occupancy_latch.y", "mode_select_occupied.u2"),
            ("mode_standby.y", "mode_select_occupied.u3"),
            ("mode_select_occupied.y", "mode_select_schedule.u1"),
            ("scheduled_occupied", "mode_select_schedule.u2"),
            ("mode_select_warmup.y", "mode_select_schedule.u3"),
            ("mode_select_schedule.y", "conditioning_state"),
        ])
        zone["notes"] += " The local CDL mode distinguishes occupied, sensor standby, warm-up, cooldown, setup, setback, and unoccupied conditions; these teaching codes are not the G36 zone-group enumeration."
