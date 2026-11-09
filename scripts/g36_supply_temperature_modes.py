"""Select the G36 multizone supply target from editable CDL operating modes.

Mode codes follow the pinned OBC OperationMode composite. The occupied/cooldown
reset retains this sample's existing outdoor-temperature and request tuning;
the mode selection follows OBC SupplyTemperature.mo.
"""


def add_g36_supply_temperature_modes(graph, port, constant, binary, switch):
    def wire(source, target):
        return {"from": source, "to": target}

    for index in (1, 2):
        name = f"ahu_{index}"
        ahu = next(block for block in graph["blocks"] if block["name"] == name)
        ahu["inputs"].append(port("operation_mode", "Integer", "code",
                                  "OBC Group Operation Mode", "Point"))
        graph["connections"].append(wire(f"zone_group_{index}.operation_mode",
                                         f"{name}.operation_mode"))
        ahu["blocks"].extend([
            {"name": "sat_mode_above_cooldown", "block_type": "Buildings.Controls.OBC.CDL.Integers.GreaterThreshold",
             "category": "elementary", "inputs": [port("u", "Integer", "code")],
             "outputs": [port("y", "Boolean")], "parameters": [{"name": "t", "type": "Integer", "value": 2}],
             "notes": "Codes above cooldown exclude the occupied cooling supply-air reset."},
            {"name": "sat_mode_occupied_or_cooldown", "block_type": "Buildings.Controls.OBC.CDL.Logical.Not",
             "category": "elementary", "inputs": [port("u", "Boolean")],
             "outputs": [port("y", "Boolean")], "notes": "OBC modes 1 and 2 use the occupied cooling supply-air reset."},
            {"name": "sat_mode_above_unoccupied", "block_type": "Buildings.Controls.OBC.CDL.Integers.GreaterThreshold",
             "category": "elementary", "inputs": [port("u", "Integer", "code")],
             "outputs": [port("y", "Boolean")], "parameters": [{"name": "t", "type": "Integer", "value": 0}],
             "notes": "Exclude the invalid mode 0 from the occupied reset."},
            binary("sat_mode_weather_valid", "And", "Boolean", "Boolean",
                   "Use the outdoor-temperature and request reset in modes 1 and 2."),
            {"name": "sat_mode_setup_code", "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
             "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "code")],
             "parameters": [{"name": "k", "type": "Integer", "value": 3}],
             "notes": "OBC setup mode is code 3."},
            {"name": "sat_mode_is_setup", "block_type": "Buildings.Controls.OBC.CDL.Integers.Equal",
             "category": "elementary", "inputs": [port("u1", "Integer", "code"), port("u2", "Integer", "code")],
             "outputs": [port("y", "Boolean")], "notes": "Setup selects the low cooling supply target."},
            {"name": "sat_mode_above_setup", "block_type": "Buildings.Controls.OBC.CDL.Integers.GreaterThreshold",
             "category": "elementary", "inputs": [port("u", "Integer", "code")],
             "outputs": [port("y", "Boolean")], "parameters": [{"name": "t", "type": "Integer", "value": 3}],
             "notes": "Warm-up and setback mode codes exceed setup."},
            {"name": "sat_mode_above_setback", "block_type": "Buildings.Controls.OBC.CDL.Integers.GreaterThreshold",
             "category": "elementary", "inputs": [port("u", "Integer", "code")],
             "outputs": [port("y", "Boolean")], "parameters": [{"name": "t", "type": "Integer", "value": 5}],
             "notes": "Freeze protection and unoccupied codes exceed setback."},
            {"name": "sat_mode_below_freeze", "block_type": "Buildings.Controls.OBC.CDL.Logical.Not",
             "category": "elementary", "inputs": [port("u", "Boolean")],
             "outputs": [port("y", "Boolean")], "notes": "Exclude freeze protection and unoccupied from the warm target."},
            binary("sat_mode_heat_valid", "And", "Boolean", "Boolean",
                   "OBC modes 4 and 5 use the warm-up or setback target."),
            binary("sat_mode_heating_permitted", "And", "Boolean", "Boolean",
                   "Permit normal heating-coil control only after supply fan proof; freeze stages override separately."),
            binary("sat_mode_cooling_permitted", "And", "Boolean", "Boolean",
                   "Permit normal cooling-coil control only after supply fan proof; freeze Stage 3 overrides separately."),
            constant("sat_mode_warm_target", 35.0, "°C", "Editable warm-up and setback supply-air target."),
            constant("sat_mode_off_target", 26.0, "°C", "Deadband supply-air target while the fan is off or mode is inactive."),
            switch("sat_mode_weather_select", "°C", "Use the sampled weather/request reset only in modes 1 and 2."),
            switch("sat_mode_setup_select", "°C", "Use the editable minimum cooling target in setup mode."),
            switch("sat_mode_heat_select", "°C", "Use 35 °C in warm-up and setback modes."),
            switch("sat_mode_fan_select", "°C", "Use the deadband target unless the physical supply fan is proven on."),
        ])
        ahu["connections"].extend(wire(source, target) for source, target in [
            ("operation_mode", "sat_mode_above_cooldown.u"),
            ("sat_mode_above_cooldown.y", "sat_mode_occupied_or_cooldown.u"),
            ("operation_mode", "sat_mode_above_unoccupied.u"),
            ("sat_mode_occupied_or_cooldown.y", "sat_mode_weather_valid.u1"),
            ("sat_mode_above_unoccupied.y", "sat_mode_weather_valid.u2"),
            ("operation_mode", "sat_mode_is_setup.u1"),
            ("sat_mode_setup_code.y", "sat_mode_is_setup.u2"),
            ("operation_mode", "sat_mode_above_setup.u"),
            ("operation_mode", "sat_mode_above_setback.u"),
            ("sat_mode_above_setback.y", "sat_mode_below_freeze.u"),
            ("sat_mode_above_setup.y", "sat_mode_heat_valid.u1"),
            ("sat_mode_below_freeze.y", "sat_mode_heat_valid.u2"),
            ("sat_output.y", "sat_mode_weather_select.u1"),
            ("sat_mode_off_target.y", "sat_mode_weather_select.u2"),
            ("sat_mode_weather_valid.y", "sat_mode_weather_select.u3"),
            ("sat_minimum.y", "sat_mode_setup_select.u1"),
            ("sat_mode_weather_select.y", "sat_mode_setup_select.u2"),
            ("sat_mode_is_setup.y", "sat_mode_setup_select.u3"),
            ("sat_mode_warm_target.y", "sat_mode_heat_select.u1"),
            ("sat_mode_setup_select.y", "sat_mode_heat_select.u2"),
            ("sat_mode_heat_valid.y", "sat_mode_heat_select.u3"),
            ("sat_mode_heat_select.y", "sat_mode_fan_select.u1"),
            ("sat_mode_off_target.y", "sat_mode_fan_select.u2"),
            ("supply_fan_proven", "sat_mode_fan_select.u3"),
            ("heating_coil_needed.y", "sat_mode_heating_permitted.u1"),
            ("supply_fan_proven", "sat_mode_heating_permitted.u2"),
            ("cooling_coil_needed.y", "sat_mode_cooling_permitted.u1"),
            ("supply_fan_proven", "sat_mode_cooling_permitted.u2"),
        ])
        for edge in ahu["connections"]:
            if edge["from"] == "sat_output.y" and not edge["to"].startswith("sat_mode_"):
                edge["from"] = "sat_mode_fan_select.y"
            elif edge["from"] == "heating_coil_needed.y" and edge["to"] == "freeze_stage_one_heating_enable.u1":
                edge["from"] = "sat_mode_heating_permitted.y"
            elif edge["from"] == "cooling_coil_needed.y" and edge["to"] == "cooling_coil_enable.u3":
                edge["from"] = "sat_mode_cooling_permitted.y"
        ahu["notes"] += " Editable OBC mode selection now sets occupied/cooldown, setup, warm-up/setback, and fan-off supply targets."
