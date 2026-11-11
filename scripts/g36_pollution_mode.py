"""Editable G36-2024 outdoor-air pollution mode and economizer lockout."""
from copy import deepcopy


def add_pollution_mode(graph, port):
    def block(name, kind, inputs, output="Boolean", parameters=None, notes=""):
        result = {"name": name, "block_type": f"Buildings.Controls.OBC.CDL.{kind}",
                  "category": "elementary", "inputs": inputs,
                  "outputs": [port("y", output)], "notes": notes}
        if parameters is not None:
            result["parameters"] = parameters
        return result

    def wire(source, target):
        return {"from": source, "to": target}

    mode = {"name": "outdoor_air_pollution_mode", "block_type": "OutdoorAirPollutionMode",
            "category": "composite",
            "inputs": [port("operator", "Integer", "count"), port("on_duration_s", unit="s"),
                       port("pm25", unit="µg/m³"), port("pm25_available", "Boolean"),
                       port("ozone", unit="ppm"), port("ozone_available", "Boolean"),
                       port("aqi", unit="AQI"), port("aqi_available", "Boolean")],
            "outputs": [port("active", "Boolean"), port("level_one_alarm", "Integer", "count"),
                        port("level_three_reminder", "Integer", "count")],
            "blocks": [], "connections": [],
            "notes": "G36-2024 Section 5.1.17. Operator Off=0, timed On=1, Auto=2. Each available pollutant has an independent 30-minute assertion and release. The reminder clock is scheduled daily at 09:00 UTC."}
    blocks = mode["blocks"]
    edges = mode["connections"]
    def add(*items):
        blocks.extend(items)
    def connect(*pairs):
        edges.extend(wire(a, b) for a, b in pairs)
    integer = lambda name, value: block(name, "Integers.Sources.Constant", [], "Integer",
                                        [{"name": "k", "type": "Integer", "value": value}],
                                        f"Operator switch code {value}.")
    delay = lambda name, seconds: block(name, "Logical.TrueDelay", [port("u", "Boolean")],
                                        parameters=[{"name": "delayTime", "value": seconds, "unit": "s"},
                                                    {"name": "delayOnInit", "type": "Boolean", "value": True}],
                                        notes="Require continuous input for the configured duration, including at initialization.")
    add(integer("off_code", 0), integer("on_code", 1), integer("auto_code", 2),
        block("is_off", "Integers.Equal", [port("u1", "Integer"), port("u2", "Integer")]),
        block("is_on", "Integers.Equal", [port("u1", "Integer"), port("u2", "Integer")]),
        block("is_auto", "Integers.Equal", [port("u1", "Integer"), port("u2", "Integer")]),
        block("on_elapsed", "Logical.Timer", [port("u", "Boolean")], "Real",
              notes="Count continuous operator On time in seconds; reset when the switch leaves On."),
        block("on_not_expired", "Reals.Less", [port("u1", unit="s"), port("u2", unit="s")]),
        block("on_active", "Logical.And", [port("u1", "Boolean"), port("u2", "Boolean")]),
        block("auto_or_pm25", "Logical.Or", [port("u1", "Boolean"), port("u2", "Boolean")]),
        block("auto_or_ozone", "Logical.Or", [port("u1", "Boolean"), port("u2", "Boolean")]),
        block("auto_active", "Logical.And", [port("u1", "Boolean"), port("u2", "Boolean")]),
        block("on_or_auto", "Logical.Or", [port("u1", "Boolean"), port("u2", "Boolean")]),
        block("not_off", "Logical.Not", [port("u", "Boolean")]),
        block("mode_active", "Logical.And", [port("u1", "Boolean"), port("u2", "Boolean")]),
        block("alarm_level", "Conversions.BooleanToInteger", [port("u", "Boolean")], "Integer"),
        block("daily_reminder_clock", "Logical.Sources.SampleTrigger", [], "Boolean",
              [{"name": "period", "value": 86400.0, "unit": "s"},
               {"name": "shift", "value": 32400.0, "unit": "s"}],
              "Editable once-per-day reminder time, 09:00 UTC by default."),
        block("reminder_active", "Logical.And", [port("u1", "Boolean"), port("u2", "Boolean")]),
        block("reminder_level", "Conversions.BooleanToInteger", [port("u", "Boolean")], "Integer",
              [{"name": "integerTrue", "type": "Integer", "value": 3},
               {"name": "integerFalse", "type": "Integer", "value": 0}],
              "Level 3 only at the scheduled daily pulse while pollution mode is active."))
    next(item for item in blocks if item["name"] == "on_elapsed")["outputs"].append(port("passed", "Boolean"))
    connect(("operator", "is_off.u1"), ("off_code.y", "is_off.u2"),
            ("operator", "is_on.u1"), ("on_code.y", "is_on.u2"),
            ("operator", "is_auto.u1"), ("auto_code.y", "is_auto.u2"),
            ("is_on.y", "on_elapsed.u"), ("on_elapsed.y", "on_not_expired.u1"),
            ("on_duration_s", "on_not_expired.u2"),
            ("is_on.y", "on_active.u1"), ("on_not_expired.y", "on_active.u2"),
            ("pm25_enabled.y", "auto_or_pm25.u1"), ("ozone_enabled.y", "auto_or_pm25.u2"),
            ("auto_or_pm25.y", "auto_or_ozone.u1"), ("aqi_enabled.y", "auto_or_ozone.u2"),
            ("is_auto.y", "auto_active.u1"), ("auto_or_ozone.y", "auto_active.u2"),
            ("on_active.y", "on_or_auto.u1"), ("auto_active.y", "on_or_auto.u2"),
            ("is_off.y", "not_off.u"), ("on_or_auto.y", "mode_active.u1"),
            ("not_off.y", "mode_active.u2"), ("mode_active.y", "active"),
            ("mode_active.y", "alarm_level.u"), ("alarm_level.y", "level_one_alarm"),
            ("mode_active.y", "reminder_active.u1"),
            ("daily_reminder_clock.y", "reminder_active.u2"),
            ("reminder_active.y", "reminder_level.u"),
            ("reminder_level.y", "level_three_reminder"))
    for name, unit, limit in (("pm25", "µg/m³", 113.0), ("ozone", "ppm", 0.1), ("aqi", "AQI", 100.0)):
        add(block(f"{name}_limit", "Reals.Sources.Constant", [], "Real",
                  [{"name": "k", "type": "Real", "value": limit, "unit": unit}],
                  "Example limit; select for the site and AHU filtration."),
            block(f"{name}_above", "Reals.Greater", [port("u1", unit=unit), port("u2", unit=unit)]),
            block(f"{name}_below", "Reals.Less", [port("u1", unit=unit), port("u2", unit=unit)]),
            block(f"{name}_assert_available", "Logical.And", [port("u1", "Boolean"), port("u2", "Boolean")]),
            block(f"{name}_unavailable", "Logical.Not", [port("u", "Boolean")]),
            block(f"{name}_clear_or_unavailable", "Logical.Or", [port("u1", "Boolean"), port("u2", "Boolean")]),
            delay(f"{name}_assert", 1800.0), delay(f"{name}_clear", 1800.0),
            block(f"{name}_latch", "Logical.Latch", [port("u", "Boolean"), port("clr", "Boolean")]),
            block(f"{name}_enabled", "Logical.And", [port("u1", "Boolean"), port("u2", "Boolean")]))
        connect((name, f"{name}_above.u1"), (f"{name}_limit.y", f"{name}_above.u2"),
                (name, f"{name}_below.u1"), (f"{name}_limit.y", f"{name}_below.u2"),
                (f"{name}_above.y", f"{name}_assert_available.u1"),
                (f"{name}_available", f"{name}_assert_available.u2"),
                (f"{name}_assert_available.y", f"{name}_assert.u"),
                (f"{name}_below.y", f"{name}_clear.u"),
                (f"{name}_available", f"{name}_unavailable.u"),
                (f"{name}_clear.y", f"{name}_clear_or_unavailable.u1"),
                (f"{name}_unavailable.y", f"{name}_clear_or_unavailable.u2"),
                (f"{name}_assert.y", f"{name}_latch.u"),
                (f"{name}_clear_or_unavailable.y", f"{name}_latch.clr"),
                (f"{name}_latch.y", f"{name}_enabled.u1"),
                (f"{name}_available", f"{name}_enabled.u2"))
    graph["blocks"].append(mode)
    second_mode = deepcopy(mode)
    second_mode["name"] = "outdoor_air_pollution_mode_2"
    second_mode["notes"] += " This is AHU 2's independent decision; tune its PM2.5 limit for its filter."
    mode["notes"] += " This is AHU 1's independent decision; tune its PM2.5 limit for its filter."
    graph["blocks"].append(second_mode)
    for name, kind, unit in (("operator", "Integer", "count"), ("on_duration_s", "Real", "s"),
                            ("pm25", "Real", "µg/m³"), ("pm25_available", "Boolean", None),
                            ("ozone", "Real", "ppm"), ("ozone_available", "Boolean", None),
                            ("aqi", "Real", "AQI"), ("aqi_available", "Boolean", None)):
        graph["inputs"].append(port(f"pollution_{name}", kind, unit,
                                    f"Outdoor Air Pollution {name.replace('_', ' ').title()}", "Point"))
        graph["connections"].append(wire(f"pollution_{name}", f"outdoor_air_pollution_mode.{name}"))
        graph["connections"].append(wire(f"pollution_{name}", f"outdoor_air_pollution_mode_2.{name}"))
    for output, kind, unit in (("active", "Boolean", None), ("level_one_alarm", "Integer", "count"),
                               ("level_three_reminder", "Integer", "count")):
        graph["outputs"].append(port(f"pollution_{output}", kind, unit,
                                     f"Building Outdoor Air Pollution {output.replace('_', ' ').title()}", "Point"))
        for index, source in ((1, "outdoor_air_pollution_mode"), (2, "outdoor_air_pollution_mode_2")):
            graph["outputs"].append(port(f"ahu_{index}_pollution_{output}", kind, unit,
                                         f"AHU {index} Outdoor Air Pollution {output.replace('_', ' ').title()}", "Point"))
            graph["connections"].append(wire(f"{source}.{output}", f"ahu_{index}_pollution_{output}"))
    graph["blocks"].extend([
        block("pollution_any_active", "Logical.Or", [port("u1", "Boolean"), port("u2", "Boolean")],
              notes="Building pollution mode is active when either AHU's editable mode is active."),
        block("pollution_global_level_one", "Conversions.BooleanToInteger", [port("u", "Boolean")], "Integer",
              notes="Expose a building Level 1 alarm whenever either AHU's pollution mode is active."),
        block("pollution_global_reminder_clock", "Logical.Sources.SampleTrigger", [], "Boolean",
              [{"name": "period", "value": 86400.0, "unit": "s"},
               {"name": "shift", "value": 32400.0, "unit": "s"}],
              "Editable building reminder time, 09:00 UTC by default."),
        block("pollution_global_reminder_gate", "Logical.And", [port("u1", "Boolean"), port("u2", "Boolean")]),
        block("pollution_global_reminder_level", "Conversions.BooleanToInteger", [port("u", "Boolean")], "Integer",
              [{"name": "integerTrue", "type": "Integer", "value": 3},
               {"name": "integerFalse", "type": "Integer", "value": 0}]),
    ])
    graph["connections"].extend(wire(a, b) for a, b in [
        ("outdoor_air_pollution_mode.active", "pollution_any_active.u1"),
        ("outdoor_air_pollution_mode_2.active", "pollution_any_active.u2"),
        ("pollution_any_active.y", "pollution_active"),
        ("pollution_any_active.y", "pollution_global_level_one.u"),
        ("pollution_global_level_one.y", "pollution_level_one_alarm"),
        ("pollution_any_active.y", "pollution_global_reminder_gate.u1"),
        ("pollution_global_reminder_clock.y", "pollution_global_reminder_gate.u2"),
        ("pollution_global_reminder_gate.y", "pollution_global_reminder_level.u"),
        ("pollution_global_reminder_level.y", "pollution_level_three_reminder"),
    ])
    if graph.get("brick_model"):
        for name in ("operator", "on_duration_s", "pm25", "pm25_available", "ozone", "ozone_available", "aqi", "aqi_available"):
            point = f"pollution_{name}"
            graph["brick_model"]["entities"].append({
                "id": point, "class": "brick:Point", "label": f"Outdoor air pollution {name.replace('_', ' ')}",
                "connector": point,
            })
            graph["brick_model"]["relationships"].append({
                "subject": "building_01", "predicate": "brick:hasPoint", "object": point,
            })
    for index in (1, 2):
        ahu = next(item for item in graph["blocks"] if item["name"] == f"ahu_{index}")
        ahu["inputs"].append(port("pollution_active", "Boolean", label="Outdoor Air Pollution Mode", brick="Point"))
        ahu["outputs"].append(port("minimum_oa_return_direct", "Boolean", label="Minimum Outside Air Direct Return Control", brick="Point"))
        graph["outputs"].append(port(f"ahu_{index}_minimum_oa_return_direct", "Boolean", label=f"AHU {index} Minimum Outside Air Direct Return Control", brick="Point"))
        graph["connections"].append(wire(f"ahu_{index}.minimum_oa_return_direct", f"ahu_{index}_minimum_oa_return_direct"))
        ahu["blocks"].extend([
            block("pollution_clear", "Logical.Not", [port("u", "Boolean")]),
            block("pollution_clear_hold", "Logical.TrueDelay", [port("u", "Boolean")],
                  parameters=[{"name": "delayTime", "value": 600.0, "unit": "s"},
                              {"name": "delayOnInit", "type": "Boolean", "value": False}],
                  notes="Pollution immediately disables economizing; after it clears, wait ten continuous minutes before reenabling. Normal startup is eligible immediately."),
            block("economizer_pollution_gate", "Logical.And",
                  [port("u1", "Boolean"), port("u2", "Boolean")],
                  notes="Disable economizer in pollution mode and retain its ten-minute release hold; the CDL minimum outdoor-airflow loop remains active."),
            block("economizer_high_limit_sustained", "Logical.TrueDelay", [port("u", "Boolean")],
                  parameters=[{"name": "delayTime", "value": 600.0, "unit": "s"},
                              {"name": "delayOnInit", "type": "Boolean", "value": True}],
                  notes="Qualify the selected economizer high-limit exceedance for ten minutes."),
            block("economizer_high_limit_clear", "Logical.Not", [port("u", "Boolean")],
                  notes="Invert the selected raw high-limit condition before release qualification."),
            block("economizer_high_limit_clear_sustained", "Logical.TrueDelay", [port("u", "Boolean")],
                  parameters=[{"name": "delayTime", "value": 600.0, "unit": "s"},
                              {"name": "delayOnInit", "type": "Boolean", "value": True}],
                  notes="Release high-limit direct return control after ten continuous minutes below the limit."),
            block("economizer_high_limit_lockout", "Logical.Latch", [port("u", "Boolean"), port("clr", "Boolean")],
                  notes="Remember the qualified high-limit state until its ten-minute clear condition matures."),
            block("minimum_oa_direct_request", "Logical.Or", [port("u1", "Boolean"), port("u2", "Boolean")],
                  notes="Pollution mode acts immediately; high-limit exceedance acts after ten minutes."),
            block("minimum_oa_return_direct_gate", "Logical.And", [port("u1", "Boolean"), port("u2", "Boolean")],
                  notes="Direct return control requires occupied mode and proven supply fan."),
            block("minimum_oa_direct_outdoor_delay", "Logical.TrueDelay", [port("u", "Boolean")],
                  parameters=[{"name": "delayTime", "value": 15.0, "unit": "s"},
                              {"name": "delayOnInit", "type": "Boolean", "value": True}],
                  notes="After a direct minimum-air request, allow 15 seconds with the return damper fully open before releasing the normal outdoor opening floor."),
            block("minimum_oa_direct_return_delay", "Logical.TrueDelay", [port("u", "Boolean")],
                  parameters=[{"name": "delayTime", "value": 180.0, "unit": "s"},
                              {"name": "delayOnInit", "type": "Boolean", "value": True}],
                  notes="Hold return air fully open for three minutes before the minimum-air PI takes direct return-damper control."),
            block("minimum_oa_direct_base_select", "Reals.Switch", [port("u1"), port("u2"), port("u3", "Boolean")], "Real",
                  notes="In direct minimum-outdoor-air mode, remove the fixed teaching outdoor-opening floor so the measured-flow PI can reach its CDL ventilation target."),
            block("return_damper_direct_pending_select", "Reals.Switch", [port("u1"), port("u2"), port("u3", "Boolean")], "Real",
                  notes="On a direct minimum-air request, fully open return air through the three-minute transition; otherwise keep the normal SAT-limited return path."),
            block("return_damper_direct_select", "Reals.Switch", [port("u1"), port("u2"), port("u3", "Boolean")], "Real",
                  notes="After the three-minute transition, use the minimum-air PI MaxRA-P endpoint instead of the SAT-limited return command."),
        ])
        target = next(edge for edge in ahu["connections"] if edge["to"] == "economizer_enabled.u2")
        target["to"] = "economizer_pollution_gate.u1"
        outdoor_floor = next(edge for edge in ahu["connections"] if edge["to"] == "minimum_oa_command.u1")
        outdoor_floor["to"] = "minimum_oa_direct_base_select.u2"
        ahu["connections"].extend([wire("pollution_active", "pollution_clear.u"),
                                    wire("pollution_clear.y", "pollution_clear_hold.u"),
                                    wire("pollution_clear_hold.y", "economizer_pollution_gate.u2"),
                                    wire("economizer_pollution_gate.y", "economizer_enabled.u2"),
                                    wire("economizer_dry_bulb_device_select.y", "economizer_high_limit_sustained.u"),
                                    wire("economizer_dry_bulb_device_select.y", "economizer_high_limit_clear.u"),
                                    wire("economizer_high_limit_clear.y", "economizer_high_limit_clear_sustained.u"),
                                    wire("economizer_high_limit_sustained.y", "economizer_high_limit_lockout.u"),
                                    wire("economizer_high_limit_clear_sustained.y", "economizer_high_limit_lockout.clr"),
                                    wire("economizer_high_limit_lockout.y", "minimum_oa_direct_request.u1"),
                                    wire("pollution_active", "minimum_oa_direct_request.u2"),
                                    wire("minimum_oa_direct_request.y", "minimum_oa_return_direct_gate.u1"),
                                    wire("minimum_oa_mode_enabled.y", "minimum_oa_return_direct_gate.u2"),
                                    wire("minimum_oa_return_direct_gate.y", "minimum_oa_direct_outdoor_delay.u"),
                                    wire("minimum_oa_return_direct_gate.y", "minimum_oa_direct_return_delay.u"),
                                    wire("minimum_oa_return_direct_gate.y", "return_damper_direct_pending_select.u3"),
                                    wire("minimum_oa_direct_return_delay.y", "minimum_oa_return_direct"),
                                    wire("minimum_oa_direct_outdoor_delay.y", "minimum_oa_direct_base_select.u3"),
                                    wire("minimum_oa_direct_return_delay.y", "return_damper_direct_select.u3"),
                                    wire("minimum_oa_off_command.y", "minimum_oa_direct_base_select.u1"),
                                    wire("minimum_oa_direct_base_select.y", "minimum_oa_command.u1"),
                                    wire("return_damper_pi_max.y", "return_damper_direct_select.u1"),
                                    wire("return_damper_mode.y", "return_damper_direct_pending_select.u2"),
                                    wire("return_damper_one.y", "return_damper_direct_pending_select.u1"),
                                    wire("return_damper_direct_pending_select.y", "return_damper_direct_select.u2"),
                                    wire("return_damper_direct_select.y", "return_damper_physical_limit.u1")])
        direct = next(edge for edge in ahu["connections"]
                      if edge["from"] == "return_damper_mode.y" and edge["to"] == "return_damper_physical_limit.u1")
        ahu["connections"].remove(direct)
        source = "outdoor_air_pollution_mode.active" if index == 1 else "outdoor_air_pollution_mode_2.active"
        graph["connections"].append(wire(source, f"ahu_{index}.pollution_active"))
    add_local_reminder_clocks(graph, port)
    graph["notes"] += " Outdoor-air pollution mode and economizer lockout are editable CDL; raw pollutant and operator points enter from the simulation scenario."


def add_local_reminder_clocks(graph, port):
    """Give CDL a raw wall clock; keep reminder time and alarm decisions editable."""
    graph["inputs"].append(port("local_day_seconds", unit="s",
                                label="Local wall clock · seconds after midnight", brick="Point"))
    graph["inputs"][-1]["description"] = (
        "Raw local time supplied by the simulation clock; CDL selects the reminder time and alarm."
    )

    def convert(parent, clock_name):
        clock = next(block for block in parent["blocks"] if block["name"] == clock_name)
        shift = next(parameter["value"] for parameter in clock["parameters"]
                     if parameter["name"] == "shift")
        clock.update({"block_type": "Buildings.Controls.OBC.CDL.Logical.Or",
                      "inputs": [port("u1", "Boolean"), port("u2", "Boolean")],
                      "parameters": [],
                      "notes": "Pulse at the local reminder crossing, including when simulation begins exactly on it."})
        parent["blocks"].extend([
            {"name": clock_name + "_time", "block_type": "Buildings.Controls.OBC.CDL.Reals.Sources.Constant",
             "category": "elementary", "inputs": [], "outputs": [port("y")],
             "parameters": [{"name": "k", "value": shift, "unit": "s"}],
             "notes": "Editable local reminder time in seconds after midnight; 32400 is 09:00."},
            {"name": clock_name + "_before", "block_type": "Buildings.Controls.OBC.CDL.Reals.Less",
             "category": "elementary", "inputs": [port("u1", unit="s"), port("u2", unit="s")],
             "outputs": [port("y", "Boolean")],
             "notes": "True before the local reminder time; resets at local midnight."},
            {"name": clock_name + "_reached", "block_type": "Buildings.Controls.OBC.CDL.Logical.Not",
             "category": "elementary", "inputs": [port("u", "Boolean")],
             "outputs": [port("y", "Boolean")],
             "notes": "True from the local reminder time until midnight."},
            {"name": clock_name + "_edge", "block_type": "Buildings.Controls.OBC.CDL.Logical.Edge",
             "category": "elementary", "inputs": [port("u", "Boolean")],
             "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "pre_u_start", "type": "Boolean", "value": True}],
             "notes": "Detect the daily threshold crossing without a false startup pulse after 09:00."},
            {"name": clock_name + "_at_time", "block_type": "Buildings.Controls.OBC.CDL.Reals.Equal",
             "category": "elementary", "inputs": [port("u1", unit="s"), port("u2", unit="s")],
             "outputs": [port("y", "Boolean")],
             "notes": "Include a reminder when the simulation starts exactly at the scheduled local time."},
        ])
        parent["connections"].extend({"from": source, "to": target} for source, target in [
            ("local_day_seconds", clock_name + "_before.u1"),
            (clock_name + "_time.y", clock_name + "_before.u2"),
            (clock_name + "_before.y", clock_name + "_reached.u"),
            (clock_name + "_reached.y", clock_name + "_edge.u"),
            ("local_day_seconds", clock_name + "_at_time.u1"),
            (clock_name + "_time.y", clock_name + "_at_time.u2"),
            (clock_name + "_edge.y", clock_name + ".u1"),
            (clock_name + "_at_time.y", clock_name + ".u2"),
        ])

    for name in ("outdoor_air_pollution_mode", "outdoor_air_pollution_mode_2"):
        mode = next(block for block in graph["blocks"] if block["name"] == name)
        mode["inputs"].append(port("local_day_seconds", unit="s"))
        graph["connections"].append({"from": "local_day_seconds", "to": name + ".local_day_seconds"})
        convert(mode, "daily_reminder_clock")
    convert(graph, "pollution_global_reminder_clock")
    if graph.get("brick_model"):
        graph["brick_model"]["entities"].append({
            "id": "local_day_seconds", "class": "brick:Point", "label": "Local wall clock",
            "connector": "local_day_seconds",
        })
        graph["brick_model"]["relationships"].append({
            "subject": "building_01", "predicate": "brick:hasPoint", "object": "local_day_seconds",
        })
