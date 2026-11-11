"""Editable G36-2024 outdoor-air pollution mode and economizer lockout."""


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
    for name, kind, unit in (("operator", "Integer", "count"), ("on_duration_s", "Real", "s"),
                            ("pm25", "Real", "µg/m³"), ("pm25_available", "Boolean", None),
                            ("ozone", "Real", "ppm"), ("ozone_available", "Boolean", None),
                            ("aqi", "Real", "AQI"), ("aqi_available", "Boolean", None)):
        graph["inputs"].append(port(f"pollution_{name}", kind, unit,
                                    f"Outdoor Air Pollution {name.replace('_', ' ').title()}", "Point"))
        graph["connections"].append(wire(f"pollution_{name}", f"outdoor_air_pollution_mode.{name}"))
    for output, kind, unit in (("active", "Boolean", None), ("level_one_alarm", "Integer", "count"),
                               ("level_three_reminder", "Integer", "count")):
        graph["outputs"].append(port(f"pollution_{output}", kind, unit,
                                     f"Outdoor Air Pollution {output.replace('_', ' ').title()}", "Point"))
        graph["connections"].append(wire(f"outdoor_air_pollution_mode.{output}", f"pollution_{output}"))
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
        ahu["blocks"].extend([
            block("pollution_clear", "Logical.Not", [port("u", "Boolean")]),
            block("economizer_pollution_gate", "Logical.And",
                  [port("u1", "Boolean"), port("u2", "Boolean")],
                  notes="Disable economizer in pollution mode; the CDL minimum outdoor-airflow loop remains active."),
        ])
        target = next(edge for edge in ahu["connections"] if edge["to"] == "economizer_enabled.u2")
        target["to"] = "economizer_pollution_gate.u1"
        ahu["connections"].extend([wire("pollution_active", "pollution_clear.u"),
                                    wire("pollution_clear.y", "economizer_pollution_gate.u2"),
                                    wire("economizer_pollution_gate.y", "economizer_enabled.u2")])
        graph["connections"].append(wire("outdoor_air_pollution_mode.active", f"ahu_{index}.pollution_active"))
    graph["notes"] += " Outdoor-air pollution mode and economizer lockout are editable CDL; raw pollutant and operator points enter from the simulation scenario."
