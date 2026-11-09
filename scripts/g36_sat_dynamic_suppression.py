"""Use the pinned OBC request suppression duration in editable VAV CDL."""


OLD_BLOCKS = {"sat_setpoint_stable", "sat_suppression_elapsed"}
OLD_TARGETS = {"sat_two_eligible.u2", "sat_three_eligible.u2"}


def add_sat_dynamic_suppression(graph, port):
    def wire(source, target):
        return {"from": source, "to": target}

    def block(name, kind, inputs, outputs, note, parameters=None):
        result = {"name": name, "block_type": f"Buildings.Controls.OBC.CDL.{kind}",
                  "category": "elementary", "inputs": inputs, "outputs": outputs,
                  "notes": note}
        if parameters is not None:
            result["parameters"] = parameters
        return result

    def binary(name, kind, unit, output_type, output_unit, note):
        return block(name, f"Reals.{kind}", [port("u1", "Real", unit), port("u2", "Real", unit)],
                     [port("y", output_type, output_unit)], note)

    for zone in (item for item in graph["blocks"] if item["block_type"] == "VAVBoxController"):
        if not any(item["name"] == "sat_suppression_duration" for item in zone["blocks"]):
            zone["blocks"] = [item for item in zone["blocks"] if item["name"] not in OLD_BLOCKS]
            zone["connections"] = [edge for edge in zone["connections"]
                                   if edge["from"].split(".")[0] not in OLD_BLOCKS
                                   and edge["to"].split(".")[0] not in OLD_BLOCKS
                                   and edge["to"] not in OLD_TARGETS]
            zone["blocks"].extend([
                block("sat_startup_true", "Logical.Sources.Constant", [],
                      [port("y", "Boolean")], "Begin the first-sampling-period guard at initialization.",
                      [{"name": "k", "type": "Boolean", "value": True}]),
                block("sat_startup_delay", "Logical.TrueDelay", [port("u", "Boolean")],
                      [port("y", "Boolean")], "Ignore setpoint changes during the first 120 seconds, as in OBC TimeSuppression.",
                      [{"name": "delayTime", "type": "Real", "value": 120.0, "unit": "s"},
                       {"name": "delayOnInit", "type": "Boolean", "value": True}]),
                block("sat_change_after_startup", "Logical.And",
                      [port("u1", "Boolean"), port("u2", "Boolean")],
                      [port("y", "Boolean")], "Only a sampled setpoint change after startup resets suppression."),
                block("sat_change_latch", "Logical.Latch",
                      [port("u", "Boolean"), port("clr", "Boolean")],
                      [port("y", "Boolean")], "Hold the first material setpoint change until its suppression interval expires."),
                block("sat_clear_to_real", "Conversions.BooleanToReal", [port("u", "Boolean")],
                      [port("y", "Real")], "Carry the passed flag through a sampled state break."),
                block("sat_clear_previous", "Discrete.UnitDelay", [port("u", "Real")],
                      [port("y", "Real")], "Clear the suppression latch at the next 120-second sample.",
                      [{"name": "samplePeriod", "type": "Real", "value": 120.0, "unit": "s"},
                       {"name": "y_start", "type": "Real", "value": 0.0}]),
                block("sat_clear_boolean", "Reals.GreaterThreshold", [port("u", "Real")],
                      [port("y", "Boolean")], "Convert the delayed passed flag back to Boolean.",
                      [{"name": "t", "type": "Real", "value": 0.5}]),
                block("sat_latch_inactive", "Logical.Not", [port("u", "Boolean")],
                      [port("y", "Boolean")], "Requests are eligible whenever no setpoint-change interval is active."),
                block("sat_change_edge", "Logical.Edge", [port("u", "Boolean")],
                      [port("y", "Boolean")], "Capture the next material setpoint change once."),
                *[block(f"sat_change_{name}", "Discrete.TriggeredSampler",
                        [port("u", "Real", "°C"), port("trigger", "Boolean")],
                        [port("y", "Real", "°C")], f"{name} at the setpoint change.",
                        [{"name": "y_start", "type": "Real", "value": 24.0, "unit": "°C"}])
                  for name in ("setpoint", "room_temp")],
                binary("sat_change_error", "Subtract", "°C", "Real", "K",
                       "Setpoint minus room temperature when the change occurred."),
                binary("sat_change_error_reverse", "Subtract", "°C", "Real", "K",
                       "Room temperature minus setpoint when the change occurred."),
                binary("sat_change_error_abs", "Max", "K", "Real", "K",
                       "Magnitude of the zone error when the setpoint changes."),
                block("sat_suppression_rate", "Reals.MultiplyByParameter", [port("u", "Real", "K")],
                      [port("y", "Real", "s")], "OBC request suppression: 540 seconds per Kelvin of zone error.",
                      [{"name": "k", "type": "Real", "value": 540.0, "unit": "s/K"}]),
                block("sat_suppression_maximum", "Reals.Sources.Constant", [],
                      [port("y", "Real", "s")], "Cap request suppression at 30 minutes.",
                      [{"name": "k", "type": "Real", "value": 1800.0, "unit": "s"}]),
                binary("sat_suppression_duration", "Min", "s", "Real", "s",
                       "Minimum of 540 s/K times captured zone error and 1800 seconds."),
                block("sat_suppression_timer", "Logical.Timer", [port("u", "Boolean")],
                      [port("y", "Real", "s"), port("passed", "Boolean")],
                      "Elapsed time while the first setpoint change remains latched."),
                binary("sat_suppression_passed", "Greater", "s", "Boolean", None,
                       "Release after elapsed time exceeds the calculated duration."),
                block("sat_suppression_complete", "Logical.Or",
                      [port("u1", "Boolean"), port("u2", "Boolean")],
                      [port("y", "Boolean")], "Allow requests outside a latched change or once its suppression time has passed."),
            ])
            zone["connections"].extend(wire(source, target) for source, target in [
                ("sat_startup_true.y", "sat_startup_delay.u"),
                ("sat_startup_delay.y", "sat_change_after_startup.u1"),
                ("sat_setpoint_changed.y", "sat_change_after_startup.u2"),
                ("sat_change_after_startup.y", "sat_change_latch.u"),
                ("sat_clear_boolean.y", "sat_change_latch.clr"),
                ("sat_change_latch.y", "sat_change_edge.u"),
                ("sat_change_latch.y", "sat_latch_inactive.u"),
                ("sat_change_edge.y", "sat_change_setpoint.trigger"),
                ("sat_change_edge.y", "sat_change_room_temp.trigger"),
                ("window_cool_target.y", "sat_change_setpoint.u"),
                ("room_temp", "sat_change_room_temp.u"),
                ("sat_change_setpoint.y", "sat_change_error.u1"),
                ("sat_change_room_temp.y", "sat_change_error.u2"),
                ("sat_change_room_temp.y", "sat_change_error_reverse.u1"),
                ("sat_change_setpoint.y", "sat_change_error_reverse.u2"),
                ("sat_change_error.y", "sat_change_error_abs.u1"),
                ("sat_change_error_reverse.y", "sat_change_error_abs.u2"),
                ("sat_change_error_abs.y", "sat_suppression_rate.u"),
                ("sat_suppression_rate.y", "sat_suppression_duration.u1"),
                ("sat_suppression_maximum.y", "sat_suppression_duration.u2"),
                ("sat_change_latch.y", "sat_suppression_timer.u"),
                ("sat_suppression_timer.y", "sat_suppression_passed.u1"),
                ("sat_suppression_duration.y", "sat_suppression_passed.u2"),
                ("sat_suppression_passed.y", "sat_clear_to_real.u"),
                ("sat_clear_to_real.y", "sat_clear_previous.u"),
                ("sat_clear_previous.y", "sat_clear_boolean.u"),
                ("sat_latch_inactive.y", "sat_suppression_complete.u1"),
                ("sat_suppression_passed.y", "sat_suppression_complete.u2"),
                ("sat_suppression_complete.y", "sat_two_eligible.u2"),
                ("sat_suppression_complete.y", "sat_three_eligible.u2"),
            ])
            zone["notes"] += " SAT request suppression now follows the editable 540 s/K captured-error calculation, capped at 1800 s."
