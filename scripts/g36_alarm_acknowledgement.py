"""Editable acknowledgment state for each VAV low-airflow alarm."""


def add_low_airflow_acknowledgement(graph, port):
    def wire(source, target):
        return {"from": source, "to": target}

    def block(name, kind, inputs, note):
        return {"name": name, "block_type": f"Buildings.Controls.OBC.CDL.Logical.{kind}",
                "category": "elementary", "inputs": inputs,
                "outputs": [port("y", "Boolean")], "notes": note}

    for ahu in (1, 2):
        name = f"ahu_{ahu}_alarm_acknowledgement"
        group = {"name": name, "block_type": "AlarmAcknowledgement",
                 "category": "composite", "description": "Tracks operator acknowledgement of active VAV low-airflow alarms.",
                 "notes": "An operator button edge acknowledges only a currently active alarm. "
                          "The acknowledgment clears when that alarm clears; the alarm level remains visible.",
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
            raw = f"{prefix}_low_airflow_acknowledge"
            status = f"{prefix}_low_airflow_acknowledged"
            graph["inputs"].append(port(raw, "Boolean", "unitless",
                                        f"Zone {zone} Low-Airflow Operator Acknowledge Button", "Point"))
            graph["outputs"].append(port(status, "Boolean", "unitless",
                                         f"Zone {zone} Low-Airflow Alarm Acknowledged", "Status"))
            graph["connections"].extend([
                wire(f"{prefix}.low_airflow_alarm_level", f"{name}.alarm_level_{local}"),
                wire(raw, f"{name}.acknowledge_{local}"),
                wire(f"{name}.acknowledged_{local}", status),
            ])
        graph["blocks"].append(group)
    graph["notes"] += " VAV low-airflow alarm acknowledgement is tracked by editable CDL latches from raw operator button contacts."
