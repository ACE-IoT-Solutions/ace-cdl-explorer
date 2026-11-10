"""Editable acknowledgment state for each VAV alarm level."""


def add_vav_alarm_acknowledgement(graph, port):
    other_alarms = (
        "airflow_sensor_alarm",
        "low_discharge_temperature_alarm",
        "leaking_damper_alarm",
        "leaking_reheat_valve_alarm",
    )
    def wire(source, target):
        return {"from": source, "to": target}

    def block(name, kind, inputs, note):
        return {"name": name, "block_type": f"Buildings.Controls.OBC.CDL.Logical.{kind}",
                "category": "elementary", "inputs": inputs,
                "outputs": [port("y", "Boolean")], "notes": note}

    for ahu in (1, 2):
        name = f"ahu_{ahu}_alarm_acknowledgement"
        group = {"name": name, "block_type": "AlarmAcknowledgement",
                 "category": "composite", "description": "Tracks operator acknowledgement of active VAV alarm levels.",
                 "notes": "One operator button edge acknowledges each currently active zone alarm. "
                          "Each acknowledgment clears when its own alarm clears; the alarm levels remain visible.",
                 "inputs": [], "outputs": [], "blocks": [], "connections": []}
        group["blocks"].append({
            "name": "alarm_zero", "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
            "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "level")],
            "parameters": [{"name": "k", "type": "Integer", "value": 0}],
            "notes": "Zero is the inactive alarm level."
        })
        for local in range(1, 6):
            zone = (ahu - 1) * 5 + local
            prefix = f"zone_{zone:02}"
            group["inputs"].extend([
                port(f"alarm_level_{local}", "Integer", "level"),
                port(f"acknowledge_{local}", "Boolean"),
            ])
            group["outputs"].append(port(f"acknowledged_{local}", "Boolean", "unitless",
                                         f"Zone {zone} Low-Airflow Alarm Acknowledged", "Status"))
            group["blocks"].extend([
                {"name": f"{prefix}_active", "block_type": "Buildings.Controls.OBC.CDL.Integers.Greater",
                 "category": "elementary", "inputs": [port("u1", "Integer", "level"),
                                                 port("u2", "Integer", "level")],
                 "outputs": [port("y", "Boolean")],
                 "notes": "The low-airflow alarm has a nonzero priority level."},
                block(f"{prefix}_ack_edge", "Edge", [port("u", "Boolean")],
                      "Only a fresh operator button press can acknowledge an alarm."),
                block(f"{prefix}_ack_active", "And", [port("u1", "Boolean"), port("u2", "Boolean")],
                      "Accept an acknowledgement only while this alarm is active."),
                block(f"{prefix}_inactive", "Not", [port("u", "Boolean")],
                      "Clear acknowledgement when the alarm condition clears."),
                block(f"{prefix}_ack_latch", "Latch", [port("u", "Boolean"), port("clr", "Boolean")],
                      "Retain acknowledgement through the active alarm episode."),
            ])
            group["connections"].extend(wire(source, target) for source, target in [
                (f"alarm_level_{local}", f"{prefix}_active.u1"),
                ("alarm_zero.y", f"{prefix}_active.u2"),
                (f"acknowledge_{local}", f"{prefix}_ack_edge.u"),
                (f"{prefix}_ack_edge.y", f"{prefix}_ack_active.u1"),
                (f"{prefix}_active.y", f"{prefix}_ack_active.u2"),
                (f"{prefix}_active.y", f"{prefix}_inactive.u"),
                (f"{prefix}_ack_active.y", f"{prefix}_ack_latch.u"),
                (f"{prefix}_inactive.y", f"{prefix}_ack_latch.clr"),
                (f"{prefix}_ack_latch.y", f"acknowledged_{local}"),
            ])
            for alarm in other_alarms:
                input_name = f"{alarm}_level_{local}"
                output_name = f"{alarm}_acknowledged_{local}"
                active = f"{prefix}_{alarm}_active"
                accepted = f"{prefix}_{alarm}_ack_active"
                inactive = f"{prefix}_{alarm}_inactive"
                latch = f"{prefix}_{alarm}_ack_latch"
                group["inputs"].append(port(input_name, "Integer", "level"))
                group["outputs"].append(port(output_name, "Boolean", "unitless",
                                             f"Zone {zone} {alarm.replace('_', ' ').title()} Acknowledged", "Status"))
                group["blocks"].extend([
                    {"name": active, "block_type": "Buildings.Controls.OBC.CDL.Integers.Greater",
                     "category": "elementary", "inputs": [port("u1", "Integer", "level"),
                                                     port("u2", "Integer", "level")],
                     "outputs": [port("y", "Boolean")],
                     "notes": "This alarm is active at a nonzero priority level."},
                    block(accepted, "And", [port("u1", "Boolean"), port("u2", "Boolean")],
                          "Accept a new operator press only during this alarm episode."),
                    block(inactive, "Not", [port("u", "Boolean")],
                          "Clear this alarm's acknowledgement when its condition clears."),
                    block(latch, "Latch", [port("u", "Boolean"), port("clr", "Boolean")],
                          "Retain acknowledgement while this alarm remains active."),
                ])
                group["connections"].extend(wire(source, target) for source, target in [
                    (input_name, f"{active}.u1"),
                    ("alarm_zero.y", f"{active}.u2"),
                    (f"{prefix}_ack_edge.y", f"{accepted}.u1"),
                    (f"{active}.y", f"{accepted}.u2"),
                    (f"{active}.y", f"{inactive}.u"),
                    (f"{accepted}.y", f"{latch}.u"),
                    (f"{inactive}.y", f"{latch}.clr"),
                    (f"{latch}.y", output_name),
                ])
                root_status = f"{prefix}_{alarm}_acknowledged"
                graph["outputs"].append(port(root_status, "Boolean", "unitless",
                                             f"Zone {zone} {alarm.replace('_', ' ').title()} Acknowledged", "Status"))
                graph["connections"].extend([
                    wire(f"{prefix}.{alarm}_level", f"{name}.{input_name}"),
                    wire(f"{name}.{output_name}", root_status),
                ])
            raw = f"{prefix}_alarm_acknowledge"
            status = f"{prefix}_low_airflow_acknowledged"
            graph["inputs"].append(port(raw, "Boolean", "unitless",
                                        f"Zone {zone} Alarm Operator Acknowledge Button", "Point"))
            graph["outputs"].append(port(status, "Boolean", "unitless",
                                         f"Zone {zone} Low-Airflow Alarm Acknowledged", "Status"))
            graph["connections"].extend([
                wire(f"{prefix}.low_airflow_alarm_level", f"{name}.alarm_level_{local}"),
                wire(raw, f"{name}.acknowledge_{local}"),
                wire(f"{name}.acknowledged_{local}", status),
            ])
        graph["blocks"].append(group)
    graph["notes"] += " VAV alarm acknowledgements are tracked by editable CDL latches from raw per-zone operator button contacts."


def add_ahu_alarm_acknowledgement(graph, port):
    alarms = (
        "freeze_ventilation_alarm",
        "freeze_alarm",
        "supply_fan_alarm",
        "fan_speed_tracking_alarm",
        "outdoor_damper_tracking_alarm",
        "return_damper_tracking_alarm",
    )

    def wire(source, target):
        return {"from": source, "to": target}

    def logical(name, kind, inputs, note):
        return {"name": name, "block_type": f"Buildings.Controls.OBC.CDL.Logical.{kind}",
                "category": "elementary", "inputs": inputs,
                "outputs": [port("y", "Boolean")], "notes": note}

    for ahu in (1, 2):
        prefix = f"ahu_{ahu}"
        name = f"{prefix}_equipment_alarm_acknowledgement"
        raw = f"{prefix}_alarm_acknowledge"
        graph["inputs"].append(port(raw, "Boolean", "unitless",
                                    f"AHU {ahu} Alarm Operator Acknowledge Button", "Point"))
        group = {
            "name": name, "block_type": "AlarmAcknowledgement", "category": "composite",
            "description": f"Tracks independent operator acknowledgements for AHU {ahu} alarms.",
            "notes": "A fresh raw button press acknowledges each active AHU alarm. "
                     "Each status clears when its own alarm clears; alarm levels remain visible.",
            "inputs": [port("acknowledge", "Boolean")], "outputs": [],
            "blocks": [{
                "name": "alarm_zero", "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
                "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "level")],
                "parameters": [{"name": "k", "type": "Integer", "value": 0}],
                "notes": "Zero is the inactive alarm level.",
            }, logical("ack_edge", "Edge", [port("u", "Boolean")],
                       "Only a fresh operator press can acknowledge an active alarm.")],
            "connections": [wire("acknowledge", "ack_edge.u")],
        }
        graph["connections"].append(wire(raw, f"{name}.acknowledge"))
        for alarm in alarms:
            source = f"{prefix}_{alarm}_level"
            status = f"{prefix}_{alarm}_acknowledged"
            group["inputs"].append(port(source, "Integer", "level"))
            group["outputs"].append(port(status, "Boolean", "unitless",
                                         f"AHU {ahu} {alarm.replace('_', ' ').title()} Acknowledged", "Status"))
            group["blocks"].extend([
                {"name": f"{alarm}_active", "block_type": "Buildings.Controls.OBC.CDL.Integers.Greater",
                 "category": "elementary", "inputs": [port("u1", "Integer", "level"),
                                                 port("u2", "Integer", "level")],
                 "outputs": [port("y", "Boolean")],
                 "notes": "This AHU alarm has a nonzero priority level."},
                logical(f"{alarm}_ack_active", "And",
                        [port("u1", "Boolean"), port("u2", "Boolean")],
                        "Accept an operator press only while this alarm is active."),
                logical(f"{alarm}_inactive", "Not", [port("u", "Boolean")],
                        "Clear acknowledgment when this alarm condition clears."),
                logical(f"{alarm}_ack_latch", "Latch",
                        [port("u", "Boolean"), port("clr", "Boolean")],
                        "Retain acknowledgment for this active alarm episode."),
            ])
            group["connections"].extend(wire(src, dst) for src, dst in [
                (source, f"{alarm}_active.u1"),
                ("alarm_zero.y", f"{alarm}_active.u2"),
                ("ack_edge.y", f"{alarm}_ack_active.u1"),
                (f"{alarm}_active.y", f"{alarm}_ack_active.u2"),
                (f"{alarm}_active.y", f"{alarm}_inactive.u"),
                (f"{alarm}_ack_active.y", f"{alarm}_ack_latch.u"),
                (f"{alarm}_inactive.y", f"{alarm}_ack_latch.clr"),
                (f"{alarm}_ack_latch.y", status),
            ])
            graph["outputs"].append(port(status, "Boolean", "unitless",
                                         f"AHU {ahu} {alarm.replace('_', ' ').title()} Acknowledged", "Status"))
            graph["connections"].extend([
                wire(f"{prefix}.{alarm}_level", f"{name}.{source}"),
                wire(f"{name}.{status}", status),
            ])
        graph["blocks"].append(group)
    graph["notes"] += " AHU alarm acknowledgements are independent editable CDL latches driven by raw operator contacts."
