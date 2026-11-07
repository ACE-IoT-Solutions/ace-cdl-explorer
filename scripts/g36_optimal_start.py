"""Per-zone, measured-recovery optimal start built from editable CDL blocks."""


def add_learned_optimal_start(graph, port, constant, binary, switch):
    def wire(source, target):
        return {"from": source, "to": target}

    def arithmetic(name, kind, left, right, result, note):
        return {"name": name, "block_type": kind, "category": "elementary",
                "inputs": [port("u1", unit=left), port("u2", unit=right)],
                "outputs": [port("y", unit=result)], "notes": note}

    def logic(name, kind, note):
        return binary(name, kind, "Boolean", "Boolean", note)

    def comparison(name, kind, unit, note):
        return {"name": name, "block_type": kind, "category": "elementary",
                "inputs": [port("u1", unit=unit), port("u2", unit=unit)],
                "outputs": [port("y", "Boolean")], "notes": note}

    def sampler(name, unit, start, note):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Discrete.TriggeredSampler",
                "category": "elementary", "inputs": [port("u", unit=unit), port("trigger", "Boolean")],
                "outputs": [port("y", unit=unit)],
                "parameters": [{"name": "y_start", "value": start, "unit": unit}], "notes": note}

    def learner(name):
        blocks = [
            constant("zero_error", 0.0, "K", "Only a positive occupied-setpoint error requires recovery."),
            constant("minimum_start_error", 0.5, "K", "Ignore episodes with less than 0.5 K of initial error."),
            constant("reached_margin", 0.1, "K", "A zone within 0.1 K of setpoint has recovered."),
            constant("minimum_recovery_seconds", 120.0, "s", "Avoid division by a nearly zero recovery time."),
            arithmetic("positive_error", "Max", "K", "K", "K", "Positive zone error."),
            logic("window_active", "And", "Train while recovery observation is active and the window is closed."),
            sampler("start_error", "K", 0.0, "Capture this zone's error when its recovery episode starts."),
            {"name": "recovery_timer", "block_type": "Buildings.Controls.OBC.CDL.Logical.Timer",
             "category": "elementary", "inputs": [port("u", "Boolean")],
             "outputs": [port("y", unit="s"), port("passed", "Boolean")],
             "parameters": [{"name": "t", "value": 50400.0, "unit": "s"}],
             "notes": "Measure recovery after group start, allowing the three-hour start cap plus eleven occupied hours."},
            comparison("reached", "Less", "K", "The zone reached its occupied setpoint."),
            {"name": "not_timed_out", "block_type": "Not", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "notes": "Do not teach from a recovery that outlasted the observation window."},
            logic("reached_before_timeout", "And", "Only actual setpoint attainment within the observation window completes learning."),
            comparison("enough_error", "Greater", "K", "Only learn from a meaningful initial error."),
            logic("active_done", "And", "Only complete an active recovery episode."),
            logic("valid_completion", "And", "Require a meaningful initial error."),
            {"name": "inactive", "block_type": "Not", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "notes": "Clear the episode latch when the zone stops recovering."},
            {"name": "completion_latch", "block_type": "Buildings.Controls.OBC.CDL.Logical.Latch",
             "category": "elementary", "inputs": [port("u", "Boolean"), port("clr", "Boolean")],
             "outputs": [port("y", "Boolean")],
             "notes": "Only one learned rate is recorded per continuous recovery episode."},
            {"name": "completion_edge", "block_type": "Buildings.Controls.OBC.CDL.Logical.Edge",
             "category": "elementary", "inputs": [port("u", "Boolean")],
             "outputs": [port("y", "Boolean")], "notes": "Sample one recovery rate per completed episode."},
            arithmetic("recovery_duration", "Max", "s", "s", "s", "At least 120 seconds of recovery time."),
            arithmetic("measured_rate", "Divide", "K", "s", "K/s", "Initial temperature error divided by recovery duration."),
            switch("rate_sample", "K/s", "Feed the measured rate only on a valid completion edge."),
            sampler("latest_completed_rate", "K/s", 0.0, "Hold the most recent completed recovery rate until the daily sample."),
            constant("zero_sampled_need", 0.0, "K", "Only a positive sampled occupied-setpoint error requires optimal start."),
            comparison("start_required", "Greater", "K", "Use the sampled horizon error to qualify the daily slope sample."),
            logic("eligible_start", "And", "Open-window zones do not teach the daily recovery mean."),
            logic("daily_sample", "And", "Sample once at midnight only if optimal start was needed."),
            {"name": "rate_mean", "block_type": "Buildings.Controls.OBC.CDL.Discrete.TriggeredMovingMean",
             "category": "elementary", "inputs": [port("u", unit="K/s"), port("trigger", "Boolean")],
             "outputs": [port("y", unit="K/s")],
             "parameters": [{"name": "n", "value": 3}],
             "notes": "Average at most three qualified daily recovery rates; the separate Max block enforces the fallback minimum."},
            arithmetic("rate_floor", "Max", "K/s", "K/s", "K/s", "Keep the editable default slope as the lower bound."),
        ]
        edges = [
            wire("error", "positive_error.u1"), wire("zero_error.y", "positive_error.u2"),
            wire("active", "window_active.u1"), wire("window_closed", "window_active.u2"),
            wire("positive_error.y", "start_error.u"), wire("window_active.y", "start_error.trigger"),
            wire("window_active.y", "recovery_timer.u"),
            wire("positive_error.y", "reached.u1"), wire("reached_margin.y", "reached.u2"),
            wire("recovery_timer.passed", "not_timed_out.u"),
            wire("reached.y", "reached_before_timeout.u1"),
            wire("not_timed_out.y", "reached_before_timeout.u2"),
            wire("start_error.y", "enough_error.u1"), wire("minimum_start_error.y", "enough_error.u2"),
            wire("window_active.y", "active_done.u1"), wire("reached_before_timeout.y", "active_done.u2"),
            wire("active_done.y", "valid_completion.u1"), wire("enough_error.y", "valid_completion.u2"),
            wire("window_active.y", "inactive.u"),
            wire("valid_completion.y", "completion_latch.u"), wire("inactive.y", "completion_latch.clr"),
            wire("completion_latch.y", "completion_edge.u"),
            wire("recovery_timer.y", "recovery_duration.u1"),
            wire("minimum_recovery_seconds.y", "recovery_duration.u2"),
            wire("start_error.y", "measured_rate.u1"), wire("recovery_duration.y", "measured_rate.u2"),
            wire("measured_rate.y", "rate_sample.u1"), wire("fallback_rate", "rate_sample.u2"),
            wire("completion_edge.y", "rate_sample.u3"),
            wire("rate_sample.y", "latest_completed_rate.u"),
            wire("completion_edge.y", "latest_completed_rate.trigger"),
            wire("sampled_need", "start_required.u1"),
            wire("zero_sampled_need.y", "start_required.u2"),
            wire("start_required.y", "eligible_start.u1"),
            wire("window_closed", "eligible_start.u2"),
            wire("midnight", "daily_sample.u1"),
            wire("eligible_start.y", "daily_sample.u2"),
            wire("latest_completed_rate.y", "rate_mean.u"),
            wire("daily_sample.y", "rate_mean.trigger"),
            wire("rate_mean.y", "rate_floor.u1"), wire("fallback_rate", "rate_floor.u2"),
            wire("rate_floor.y", "rate"),
            wire("start_error.y", "sampled_start_error"),
            wire("recovery_timer.y", "recovery_seconds"),
        ]
        return {"name": name, "block_type": "ZoneRecoveryLearner", "category": "composite",
                "description": "Learn this zone's occupied-setpoint recovery slope from completed episodes.",
                "notes": "A positive occupied-setpoint error sampled at the start horizon qualifies the midnight moving-mean sample. Only measured setpoint attainment updates the held rate; open windows are excluded.",
                "inputs": [port("error", unit="K"), port("active", "Boolean"),
                           port("window_closed", "Boolean"), port("fallback_rate", unit="K/s"),
                           port("midnight", "Boolean"), port("sampled_need", unit="K")],
                "outputs": [port("rate", unit="K/s"), port("sampled_start_error", unit="K"),
                            port("recovery_seconds", unit="s")],
                "blocks": blocks, "connections": edges}

    def predictor(name):
        blocks = [
            constant("zero_error", 0.0, "K", "No recovery time is needed when the window is open."),
            constant("maximum_seconds", 10800.0, "s", "Cap the predicted start at three hours."),
            arithmetic("positive_error", "Max", "K", "K", "K", "Use only positive occupied-setpoint error."),
            logic("sample_enabled", "And", "Sample at the three-hour horizon or when a window closes within it."),
            sampler("horizon_error", "K", 0.0, "Hold the zone error sampled at the start horizon."),
            switch("closed_error", "K", "Open windows contribute zero predicted recovery time."),
            arithmetic("time_from_rate", "Divide", "K", "K/s", "s", "Sampled error divided by the learned zone slope."),
            arithmetic("time_capped", "Min", "s", "s", "s", "Limit the zone's predicted start time."),
        ]
        edges = [
            wire("error", "positive_error.u1"), wire("zero_error.y", "positive_error.u2"),
            wire("sample_horizon", "sample_enabled.u1"), wire("window_closed", "sample_enabled.u2"),
            wire("positive_error.y", "horizon_error.u"), wire("sample_enabled.y", "horizon_error.trigger"),
            wire("horizon_error.y", "closed_error.u1"), wire("zero_error.y", "closed_error.u2"),
            wire("window_closed", "closed_error.u3"),
            wire("closed_error.y", "time_from_rate.u1"), wire("rate", "time_from_rate.u2"),
            wire("time_from_rate.y", "time_capped.u1"), wire("maximum_seconds.y", "time_capped.u2"),
            wire("time_capped.y", "seconds"), wire("horizon_error.y", "sampled_error"),
        ]
        return {"name": name, "block_type": "ZoneOptimalStartPrediction", "category": "composite",
                "description": "Predict this zone's start time from its sampled error and learned recovery slope.",
                "notes": "Samples once at the three-hour horizon and again if a previously open window closes during that horizon.",
                "inputs": [port("error", unit="K"), port("sample_horizon", "Boolean"),
                           port("window_closed", "Boolean"), port("rate", unit="K/s")],
                "outputs": [port("seconds", unit="s"), port("sampled_error", unit="K")],
                "blocks": blocks, "connections": edges}

    for group_index in (1, 2):
        group = next(block for block in graph["blocks"] if block["name"] == f"zone_group_{group_index}")
        blocks, edges = group["blocks"], group["connections"]
        blocks.append(constant("optimal_one_kelvin", 1.0, "K", "Convert editable seconds per kelvin into the default K/s recovery slope."))
        blocks.append(constant("optimal_minimum_seconds_per_k", 120.0, "s", "Keep the editable fallback denominator positive."))
        blocks.append({"name": "optimal_daily_sample", "block_type": "Buildings.Controls.OBC.CDL.Logical.Sources.SampleTrigger",
                       "category": "elementary", "inputs": [], "outputs": [port("y", "Boolean")],
                       "parameters": [{"name": "period", "value": 86400.0, "unit": "s"},
                                      {"name": "shift", "value": 0.0, "unit": "s"}],
                       "notes": "Sample each completed zone recovery at midnight, as in the OBC optimal-start calculation."})
        for mode in ("warmup", "cooldown"):
            fallback = f"optimal_{mode}_fallback_rate"
            safe_seconds = f"optimal_{mode}_seconds_safe"
            horizon = f"optimal_{mode}_horizon"
            active = f"optimal_{mode}_active"
            blocks.extend([
                arithmetic(safe_seconds, "Max", "s", "s", "s", "Bound the fallback recovery time per kelvin away from zero."),
                arithmetic(fallback, "Divide", "K", "s", "K/s", "Editable initial recovery slope, 1 K divided by seconds per K."),
                comparison(horizon, "Less", "s", "Sample zone error at the three-hour lookahead boundary."),
                {"name": f"optimal_{mode}_horizon_hold", "block_type": "Buildings.Controls.OBC.CDL.Logical.TrueFalseHold",
                 "category": "elementary", "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
                 "parameters": [{"name": "trueHoldDuration", "value": 50400.0, "unit": "s"},
                                {"name": "falseHoldDuration", "value": 0.0, "unit": "s"}],
                 "notes": "Hold the three-hour calculation trigger through eleven more hours, following the OBC optimal-start observation window."},
                {"name": f"optimal_{mode}_observation_finished", "block_type": "Not", "category": "elementary",
                 "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
                 "notes": "Clear the recovery episode after the occupied schedule ends."},
                {"name": active, "block_type": "Buildings.Controls.OBC.CDL.Logical.Latch", "category": "elementary",
                 "inputs": [port("u", "Boolean"), port("clr", "Boolean")],
                 "outputs": [port("y", "Boolean")],
                 "notes": "Only an actual optimal-start request opens a recovery episode; hold it into occupied operation."},
            ])
            edges[:] = [edge for edge in edges if edge["to"] not in
                        (f"{mode}_seconds_per_k.u", f"{mode}_time_capped.u1")]
            edges.extend([
                wire("optimal_one_kelvin.y", f"{mode}_seconds_per_k.u"),
                wire("optimal_one_kelvin.y", f"{fallback}.u1"),
                wire(f"{mode}_seconds_per_k.y", f"{safe_seconds}.u1"),
                wire("optimal_minimum_seconds_per_k.y", f"{safe_seconds}.u2"),
                wire(f"{safe_seconds}.y", f"{fallback}.u2"),
                wire("seconds_until_occupied", f"{horizon}.u1"),
                wire("maximum_start_seconds.y", f"{horizon}.u2"),
                wire(f"{horizon}.y", f"optimal_{mode}_horizon_hold.u"),
                wire(f"optimal_{mode}_horizon_hold.y", f"optimal_{mode}_observation_finished.u"),
                wire(f"{mode}_selected.y", f"{active}.u"),
                wire(f"optimal_{mode}_observation_finished.y", f"{active}.clr"),
            ])
            previous_time = None
            for zone in range(1, 6):
                prefix = f"optimal_{mode}_{zone}"
                learner_name = f"{prefix}_learner"
                previous_name = f"{prefix}_rate_previous"
                rate_name = f"{prefix}_rate_safe"
                predictor_name = f"{prefix}_predictor"
                blocks.extend([
                    learner(learner_name),
                    {"name": previous_name, "block_type": "Buildings.Controls.OBC.CDL.Discrete.UnitDelay",
                     "category": "elementary", "inputs": [port("u", unit="K/s")],
                     "outputs": [port("y", unit="K/s")],
                     "parameters": [{"name": "samplePeriod", "value": 60.0, "unit": "s"},
                                    {"name": "y_start", "value": 0.0, "unit": "K/s"}],
                     "notes": "Break the start-decision-to-measured-rate feedback loop."},
                    arithmetic(rate_name, "Max", "K/s", "K/s", "K/s", "Use the editable fallback until a rate has been learned."),
                    predictor(predictor_name),
                ])
                edges.extend([
                    wire(f"{mode}_error_{zone}.y", f"{learner_name}.error"),
                    wire(f"{active}.y", f"{learner_name}.active"),
                    wire(f"window_closed_{zone}", f"{learner_name}.window_closed"),
                    wire(f"{fallback}.y", f"{learner_name}.fallback_rate"),
                    wire("optimal_daily_sample.y", f"{learner_name}.midnight"),
                    wire(f"{predictor_name}.sampled_error", f"{learner_name}.sampled_need"),
                    wire(f"{learner_name}.rate", f"{previous_name}.u"),
                    wire(f"{previous_name}.y", f"{rate_name}.u1"),
                    wire(f"{fallback}.y", f"{rate_name}.u2"),
                    wire(f"{mode}_error_{zone}.y", f"{predictor_name}.error"),
                    wire(f"{horizon}.y", f"{predictor_name}.sample_horizon"),
                    wire(f"window_closed_{zone}", f"{predictor_name}.window_closed"),
                    wire(f"{rate_name}.y", f"{predictor_name}.rate"),
                ])
                candidate = f"{predictor_name}.seconds"
                if previous_time is None:
                    previous_time = candidate
                else:
                    maximum = f"optimal_{mode}_longest_{zone}"
                    blocks.append(arithmetic(maximum, "Max", "s", "s", "s", "Use the longest predicted start among the group zones."))
                    edges.extend([wire(previous_time, f"{maximum}.u1"), wire(candidate, f"{maximum}.u2")])
                    previous_time = f"{maximum}.y"
            edges.append(wire(previous_time, f"{mode}_time_capped.u1"))
        group["notes"] = (
            "Each closed-window zone samples its error at the start horizon. A CDL latch opens a "
            "recovery episode only after optimal start is requested, then holds observation into "
            "occupied operation. CDL samples the held recovery rate at midnight when the sampled horizon error calls for optimal start, and learns a three-day slope from measured setpoint "
            "attainment within fourteen hours, and uses the longest zone prediction. "
            "The editable seconds-per-kelvin constants initialize and bound the learned slopes. "
            "A sampled prior rate explicitly breaks the group feedback loop. Full G36 group mode "
            "coordination and reference-trace verification remain to be added."
        )
    graph["notes"] += " CDL per-zone recovery learners replace fixed-rate optimal-start predictions; Rust supplies only schedule time and measured zone temperatures."
