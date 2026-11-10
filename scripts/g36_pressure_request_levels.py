"""Add editable 0–3 VAV duct-pressure requests and low-flow suppression."""


def add_pressure_request_levels(graph, port):
    def wire(source, target):
        return {"from": source, "to": target}

    def constant(name, value, unit, note):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Reals.Sources.Constant",
                "category": "elementary", "inputs": [], "outputs": [port("y", "Real", unit)],
                "parameters": [{"name": "k", "type": "Real", "value": value, "unit": unit}],
                "notes": note}

    def binary(name, kind, unit, note, hysteresis=None):
        block = {"name": name, "block_type": f"Buildings.Controls.OBC.CDL.Reals.{kind}" if kind in ("Greater", "Less")
                else f"Buildings.Controls.OBC.CDL.Logical.{kind}",
                "category": "elementary",
                "inputs": [port("u1", "Real" if kind in ("Greater", "Less") else "Boolean", unit if kind in ("Greater", "Less") else None),
                           port("u2", "Real" if kind in ("Greater", "Less") else "Boolean", unit if kind in ("Greater", "Less") else None)],
                "outputs": [port("y", "Boolean")], "notes": note}
        if hysteresis is not None:
            block["parameters"] = [{"name": "h", "type": "Real", "value": hysteresis, "unit": unit}]
        return block

    def multiply(name, factor, note):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Reals.MultiplyByParameter",
                "category": "elementary", "inputs": [port("u", "Real", "normalized airflow")],
                "outputs": [port("y", "Real", "normalized airflow")],
                "parameters": [{"name": "k", "type": "Real", "value": factor}], "notes": note}

    def sample(name, unit, note):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Discrete.Sampler",
                "category": "elementary", "inputs": [port("u", "Real", unit)],
                "outputs": [port("y", "Real", unit)],
                "parameters": [{"name": "samplePeriod", "type": "Real", "value": 120.0, "unit": "s"}],
                "notes": note}

    def level(name, number):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
                "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "count")],
                "parameters": [{"name": "k", "type": "Integer", "value": number}],
                "notes": f"{number} duct-pressure reset requests."}

    def select(name, note):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Integers.Switch",
                "category": "elementary",
                "inputs": [port("u1", "Integer", "count"), port("u2", "Boolean"),
                           port("u3", "Integer", "count")],
                "outputs": [port("y", "Integer", "count")], "notes": note}

    old = {"pressure_request_damper_threshold", "damper_nearly_open",
           "pressure_request_flow_fraction", "pressure_request_low_flow",
           "pressure_request_condition", "pressure_request_delay", "pressure_request_integer"}
    changed = False
    for zone in (block for block in graph["blocks"] if block["block_type"] == "VAVBoxController"):
        if any(block["name"] == "pressure_importance" for block in zone["blocks"]):
            continue
        names = {block["name"] for block in zone["blocks"]}
        if not old <= names or "low_flow_occupied_and_fan" not in names:
            raise ValueError(f"{zone['name']}: expected existing pressure and low-flow branches")
        changed = True
        zone["blocks"] = [block for block in zone["blocks"] if block["name"] not in old]
        zone["connections"] = [edge for edge in zone["connections"]
                               if edge["from"].split(".")[0] not in old
                               and edge["to"].split(".")[0] not in old
                               and (edge["from"], edge["to"]) not in {
                                   ("low_flow_occupied_and_fan.y", "low_flow_eligible.u1"),
                                   ("low_flow_occupied_and_fan.y", "low_flow_occupied_gate.u1")}]
        zone["blocks"].extend([
            constant("pressure_importance", 1.0, "fraction (0–1)",
                     "Zero removes this zone from duct-pressure reset and suppresses its low-airflow alarm."),
            constant("pressure_importance_zero", 0.0, "fraction (0–1)", "Disabled-importance threshold."),
            binary("pressure_importance_positive", "Greater", "fraction (0–1)",
                   "Only a positive pressure-reset importance enables requests and the related low-flow alarm."),
            {"name": "pressure_damper_hysteresis", "block_type": "Buildings.Controls.OBC.CDL.Reals.GreaterThreshold",
             "category": "elementary", "inputs": [port("u", "Real", "fraction (0–1)")],
             "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "t", "type": "Real", "value": 0.95},
                            {"name": "h", "type": "Real", "value": 0.1}],
             "notes": "OBC's shared damper latch starts above 95% and clears at or below 85%; it qualifies every request tier."},
            sample("pressure_damper_sample", "fraction (0–1)",
                   "Sample damper command at the same 120-second period as AHU pressure trim and respond."),
            sample("pressure_setpoint_sample", "normalized airflow",
                   "Sample active primary-airflow setpoint with the damper command."),
            sample("pressure_measured_sample", "normalized airflow",
                   "Sample measured primary airflow for the same pressure-request instant."),
            {"name": "pressure_damper_delay", "block_type": "Buildings.Controls.OBC.CDL.Logical.TrueDelay",
             "category": "elementary", "inputs": [port("u", "Boolean")],
             "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "delayTime", "type": "Real", "value": 60.0, "unit": "s"},
                            {"name": "delayOnInit", "type": "Boolean", "value": False}],
             "notes": "OBC waits one minute after the sampled damper request, independently of changing airflow."},
            {"name": "pressure_setpoint_positive", "block_type": "Buildings.Controls.OBC.CDL.Reals.GreaterThreshold",
             "category": "elementary", "inputs": [port("u", "Real", "normalized airflow")],
             "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "t", "type": "Real", "value": 0.01, "unit": "normalized airflow"},
                            {"name": "h", "type": "Real", "value": 0.005, "unit": "normalized airflow"}],
             "notes": "OBC permits high-tier requests only when sampled setpoint exceeds 0.01, clearing below 0.005."},
            multiply("pressure_flow_half", 0.5, "Severe request threshold is 50% of active airflow setpoint."),
            multiply("pressure_flow_seventy", 0.7, "Moderate request threshold is 70% of active airflow setpoint."),
            binary("pressure_below_half", "Less", "normalized airflow", "Measured primary airflow is below 50% of setpoint with 0.01 flow hysteresis.", 0.01),
            binary("pressure_below_seventy", "Less", "normalized airflow", "Measured primary airflow is below 70% of setpoint with 0.01 flow hysteresis.", 0.01),
            binary("pressure_damper_flow_eligible", "And", None,
                   "The sampled damper has requested pressure for one minute and airflow setpoint is positive."),
            binary("pressure_severe_eligible", "And", None, "The severe flow deficit has a nonzero setpoint."),
            binary("pressure_moderate_eligible", "And", None, "The moderate flow deficit has a nonzero setpoint."),
            *(level(f"pressure_level_{number}", number) for number in range(4)),
            select("pressure_one_request", "Send one request while the damper hysteresis is active."),
            select("pressure_two_requests", "Two requests take precedence after a sustained 70% flow deficit."),
            select("pressure_three_requests", "Three requests take precedence after a sustained 50% flow deficit."),
            select("pressure_importance_select", "A zero importance suppresses all pressure requests."),
            binary("low_flow_importance_gate", "And", None,
                   "A zero pressure-reset importance suppresses both low-airflow alarm levels."),
        ])
        zone["connections"].extend(wire(source, target) for source, target in [
            ("pressure_importance.y", "pressure_importance_positive.u1"),
            ("pressure_importance_zero.y", "pressure_importance_positive.u2"),
            ("heating_damper_select.y", "pressure_damper_sample.u"),
            ("pressure_damper_sample.y", "pressure_damper_hysteresis.u"),
            ("pressure_damper_hysteresis.y", "pressure_damper_delay.u"),
            ("airflow_request_sum.y", "pressure_setpoint_sample.u"),
            ("pressure_setpoint_sample.y", "pressure_setpoint_positive.u"),
            ("pressure_setpoint_sample.y", "pressure_flow_half.u"),
            ("pressure_setpoint_sample.y", "pressure_flow_seventy.u"),
            ("measured_airflow", "pressure_measured_sample.u"),
            ("pressure_measured_sample.y", "pressure_below_half.u1"),
            ("pressure_flow_half.y", "pressure_below_half.u2"),
            ("pressure_measured_sample.y", "pressure_below_seventy.u1"),
            ("pressure_flow_seventy.y", "pressure_below_seventy.u2"),
            ("pressure_damper_delay.y", "pressure_damper_flow_eligible.u1"),
            ("pressure_setpoint_positive.y", "pressure_damper_flow_eligible.u2"),
            ("pressure_damper_flow_eligible.y", "pressure_severe_eligible.u1"),
            ("pressure_below_half.y", "pressure_severe_eligible.u2"),
            ("pressure_damper_flow_eligible.y", "pressure_moderate_eligible.u1"),
            ("pressure_below_seventy.y", "pressure_moderate_eligible.u2"),
            ("pressure_level_1.y", "pressure_one_request.u1"),
            ("pressure_damper_hysteresis.y", "pressure_one_request.u2"),
            ("pressure_level_0.y", "pressure_one_request.u3"),
            ("pressure_level_2.y", "pressure_two_requests.u1"),
            ("pressure_moderate_eligible.y", "pressure_two_requests.u2"),
            ("pressure_one_request.y", "pressure_two_requests.u3"),
            ("pressure_level_3.y", "pressure_three_requests.u1"),
            ("pressure_severe_eligible.y", "pressure_three_requests.u2"),
            ("pressure_two_requests.y", "pressure_three_requests.u3"),
            ("pressure_three_requests.y", "pressure_importance_select.u1"),
            ("pressure_importance_positive.y", "pressure_importance_select.u2"),
            ("pressure_level_0.y", "pressure_importance_select.u3"),
            ("pressure_importance_select.y", "pressure_reset_request"),
            ("low_flow_occupied_and_fan.y", "low_flow_importance_gate.u1"),
            ("pressure_importance_positive.y", "low_flow_importance_gate.u2"),
            ("low_flow_importance_gate.y", "low_flow_eligible.u1"),
            ("low_flow_importance_gate.y", "low_flow_occupied_gate.u1"),
        ])
        zone["notes"] += " Duct-pressure reset samples damper, setpoint, and measured flow together every 120 seconds; one minute of damper request qualifies the 2/3 levels. A zero importance suppresses requests and low-airflow alarms."
    if changed:
        graph["notes"] += " Duct-pressure reset receives 0–3 zone requests from editable CDL rather than a binary teaching shortcut."
