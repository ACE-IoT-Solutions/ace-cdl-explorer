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


def add_g36_obc_supply_air_reset(graph, port, constant):
    """Use the OBC sampled maximum and linear 16–21 °C outdoor reset."""
    legacy_weather = {"supply_reset", "warmup_supply", "mild_or_cool", "mild_supply",
                      "cooling_supply", "is_cold", "is_mild", "cold_threshold", "warm_threshold"}
    for index in (1, 2):
        ahu = next(block for block in graph["blocks"] if block["name"] == f"ahu_{index}")
        blocks = {block["name"]: block for block in ahu["blocks"]}
        ahu["blocks"] = [block for block in ahu["blocks"] if block["name"] not in legacy_weather]
        blocks["sat_minimum"]["parameters"][0]["value"] = 12.0
        blocks["sat_minimum"]["notes"] = "OBC lowest cooling supply target and the high-outdoor end of the reset line."
        next(parameter for parameter in blocks["sat_previous"]["parameters"] if parameter["name"] == "y_start")["value"] = 18.0
        next(parameter for parameter in blocks["sat_fan_delay"]["parameters"] if parameter["name"] == "delayTime")["value"] = 720.0
        blocks["sat_fan_delay"]["notes"] = "Delay trim and respond until 600 seconds plus one 120-second sample after physical fan proof."
        blocks["sat_below_weather_max"]["notes"] = "Clamp the sampled cooling maximum at the editable upper setpoint."
        blocks["sat_candidate"]["notes"] = "Hold the maximum until fan proof and startup delay, then use sampled trim and respond."
        blocks["sat_output"]["notes"] = "Expose the current maximum cooling supply target, or its initial value while fan proof is absent."
        ahu["blocks"].extend([
            constant("sat_maximum", 18.0, "°C", "OBC highest cooling supply target and reset initial value."),
            constant("sat_outdoor_low", 16.0, "°C", "Lower OBC outdoor reset endpoint."),
            constant("sat_outdoor_high", 21.0, "°C", "Upper OBC outdoor reset endpoint."),
            {"name": "sat_request_sample", "block_type": "Buildings.Controls.OBC.CDL.Discrete.Sampler",
             "category": "elementary", "inputs": [port("u", unit="count")],
             "outputs": [port("y", unit="count")],
             "parameters": [{"name": "samplePeriod", "value": 120.0, "unit": "s"}],
             "notes": "Sample the total VAV reset requests every two minutes, as in OBC TrimAndRespond."},
            {"name": "sat_weather_line", "block_type": "Buildings.Controls.OBC.CDL.Reals.Line",
             "category": "elementary",
             "inputs": [port(name, unit="°C") for name in ("x1", "f1", "u", "x2", "f2")],
             "outputs": [port("y", unit="°C")],
             "parameters": [{"name": "limitBelow", "type": "Boolean", "value": True},
                            {"name": "limitAbove", "type": "Boolean", "value": True}],
             "notes": "Interpolate from the sampled maximum at 16 °C outdoor to the minimum at 21 °C."},
        ])
        ahu["connections"] = [edge for edge in ahu["connections"]
                              if edge["from"].split(".")[0] not in legacy_weather
                              and edge["to"].split(".")[0] not in legacy_weather]
        substitutions = {
            ("sat_request_real.y", "sat_excess_requests.u1"): ("sat_request_sample.y", "sat_excess_requests.u1"),
            ("fan_requested.y", "sat_fan_delay.u"): ("supply_fan_proven", "sat_fan_delay.u"),
            ("sat_previous.y", "sat_output.u1"): ("sat_candidate.y", "sat_output.u1"),
            ("sat_output.y", "sat_mode_weather_select.u1"): ("sat_weather_line.y", "sat_mode_weather_select.u1"),
        }
        for edge in ahu["connections"]:
            key = (edge["from"], edge["to"])
            if key in substitutions:
                edge["from"], edge["to"] = substitutions[key]
        ahu["connections"].extend({"from": source, "to": target} for source, target in [
            ("sat_request_real.y", "sat_request_sample.u"),
            ("sat_maximum.y", "sat_below_weather_max.u2"),
            ("sat_maximum.y", "sat_candidate.u2"),
            ("sat_maximum.y", "sat_output.u2"),
            ("sat_outdoor_low.y", "sat_weather_line.x1"),
            ("sat_output.y", "sat_weather_line.f1"),
            ("outdoor_c", "sat_weather_line.u"),
            ("sat_outdoor_high.y", "sat_weather_line.x2"),
            ("sat_minimum.y", "sat_weather_line.f2"),
        ])
        ahu["notes"] += " The cooling maximum is sampled independently of outdoor weather; a CDL Line resets it over 16–21 °C."
