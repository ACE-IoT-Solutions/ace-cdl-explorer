"""Build the occupancy-aware four-pipe example from the conventional graph."""

import copy
import json
from pathlib import Path
from g36_zone_modes import add_zone_conditioning_modes
from g36_zone_groups import add_zone_groups
from g36_zone_status import add_obc_zone_status_to_groups
from g36_operation_mode import add_obc_operation_mode_to_groups
from g36_group_status import add_obc_group_status_to_groups
from g36_group_unoccupied import add_group_unoccupied_modes
from g36_windows import add_window_control
from g36_overrides import add_group_overrides
from g36_optimal_start import add_learned_optimal_start
from g36_terminal_airflow_alarms import add_terminal_airflow_alarms
from g36_pressure_request_levels import add_pressure_request_levels
from g36_sat_request_levels import add_sat_request_levels
from g36_sat_dynamic_suppression import add_sat_dynamic_suppression
from g36_supply_temperature_modes import add_g36_supply_temperature_modes, add_g36_obc_supply_air_reset
from g36_terminal_hot_request_levels import add_terminal_hot_request_levels
from g36_ahu_plant_requests import add_ahu_plant_requests
from g36_airflow_sensor_alarm import add_airflow_sensor_alarms
from g36_low_discharge_alarm import add_low_discharge_alarms
from g36_terminal_leak_alarms import add_terminal_leak_alarms
from g36_supply_fan_alarms import add_supply_fan_alarms
from g36_fan_speed_feedback import add_fan_speed_feedback
from g36_ahu_damper_feedback import add_ahu_damper_feedback
from g36_economizer_hold import add_economizer_high_limit_hold
from g36_economizer_fixed_dry_bulb import add_fixed_dry_bulb_high_limit
from g36_economizer_disable_transition import add_economizer_disable_transition
from integrate_obc_optimal_start import integrate_graph as integrate_obc_optimal_start

EXAMPLES = Path(__file__).resolve().parents[1] / "web/public/examples"
source = json.loads((EXAMPLES / "four_pipe_building.json").read_text())
graph = copy.deepcopy(source)
graph["name"] = "FourPipeG36Controls"
# G36 replaces the conventional fixed-fraction damper pair with its own
# independently modulating outdoor/return sequence below.
for ahu_name in ("ahu_1", "ahu_2"):
    ahu_source = next(block for block in graph["blocks"] if block["name"] == ahu_name)
    conventional_outputs = {"outdoor_air_damper_position", "return_air_damper_position"}
    conventional_blocks = {"conventional_return_one", "conventional_return_complement"}
    ahu_source["outputs"] = [item for item in ahu_source["outputs"]
                             if item["name"] not in conventional_outputs]
    ahu_source["blocks"] = [item for item in ahu_source["blocks"]
                            if item["name"] not in conventional_blocks]
    ahu_source["connections"] = [edge for edge in ahu_source["connections"]
                                 if edge["from"].split(".")[0] not in conventional_blocks
                                 and edge["to"].split(".")[0] not in conventional_blocks
                                 and edge["to"] not in conventional_outputs]
    graph["outputs"] = [item for item in graph["outputs"]
                        if item["name"] not in {f"{ahu_name}_{output}" for output in conventional_outputs}]
    graph["connections"] = [edge for edge in graph["connections"]
                            if edge["to"] not in {f"{ahu_name}_{output}" for output in conventional_outputs}]
# This sample adds measured-flow proof below. Keep the conventional loop-enable
# switches out of its source path so the G36 proof gates remain the sole final
# CDL qualification of source commands.
plant_source = next(block for block in graph["blocks"] if block["name"] == "plant")
conventional_source_gates = {"heat_pump_heating_enable_gate", "heat_pump_cooling_enable_gate",
                             "boiler_enable_gate", "chiller_enable_gate"}
plant_source["blocks"] = [block for block in plant_source["blocks"]
                          if block["name"] not in conventional_source_gates]
plant_source["connections"] = [edge for edge in plant_source["connections"]
                               if edge["from"].split(".")[0] not in conventional_source_gates
                               and edge["to"].split(".")[0] not in conventional_source_gates]
plant_source["connections"].extend({"from": f"{command}.y", "to": output} for command, output in (
    ("heat_pump_heating", "heat_pump_heating_kw"), ("heat_pump_cooling", "heat_pump_cooling_kw"),
    ("boiler", "boiler_kw"), ("chiller", "chiller_kw")))
graph["description"] = "G36-style zone, AHU, and hot/chilled-water reset controls on the same simplified four-pipe building."
graph["notes"] = (
    "Compare this sample with FourPipeBuildingControls using the same weather, equipment, "
    "and physical model. Each zone has a debounced Brick occupancy sensor. Occupied setpoints "
    "are 21/24 °C, standby widens them by 0.5 K, and unoccupied setback/setup targets are 18/32 °C. "
    "Ventilation minimums and AHU fan minimums also reset. Qualified VAV cooling requests "
    "drive a sampled AHU supply-air temperature reset. Two five-zone CDL group controllers "
    "coordinate optimal start within three hours of occupancy, shared unoccupied setback/setup, "
    "and a zone freeze setback alarm. The AHUs "
    "use outdoor/return temperature comparisons and measured mixed-air feedback to modulate free cooling. "
    "Measured primary airflow and discharge temperature feed an editable terminal discharge "
    "setpoint and valve loop, plus editable measured-feedback terminal alarms. Timed Stage 1–3 AHU freeze "
    "protection overrides air delivery and requests water-side pumps; Stage 2 interrupts "
    "outdoor ventilation for one hour after a deeper fault, while Stage 3 latches shutdown. This is a G36-inspired subset; "
    "full G36 economizer limits, plant staging, alarms, and CO₂ sequence validation remain."
)


def port(name, kind="Real", unit=None, label=None, brick=None, description=None):
    result = {"name": name, "type": kind}
    if label:
        result["label"] = label
    if unit:
        result["unit"] = unit
    if description:
        result["description"] = description
    if brick:
        result["brick_class"] = brick
    return result


def constant(name, value, unit, note):
    return {
        "name": name, "block_type": "Constant", "category": "elementary",
        "inputs": [], "outputs": [port("y", unit=unit)],
        "parameters": [{"name": "k", "type": "Real", "value": value, "unit": unit}],
        "notes": note,
    }


def switch(name, unit, note):
    return {
        "name": name, "block_type": "Switch", "category": "elementary",
        "inputs": [port("u1", unit=unit), port("u2", unit=unit), port("u3", "Boolean")],
        "outputs": [port("y", unit=unit)], "notes": note,
    }


def binary(name, kind, input_type="Real", output_type="Boolean", note=""):
    return {
        "name": name, "block_type": kind, "category": "elementary",
        "inputs": [port("u1", input_type), port("u2", input_type)],
        "outputs": [port("y", output_type)], "notes": note,
    }


def add_capacity_aware_heat_pump_dispatch():
    """Allocate measured equipment capacity in CDL, then send unmet load to backups."""
    plant = next(block for block in graph["blocks"] if block["name"] == "plant")
    removed = {"heat_pump_dispatch_fraction", "heat_pump_dispatch_limit", "backup_fraction"}
    plant["blocks"] = [block for block in plant["blocks"] if block["name"] not in removed]
    plant["connections"] = [edge for edge in plant["connections"]
                            if edge["from"].split(".")[0] not in removed
                            and edge["to"].split(".")[0] not in removed
                            and edge["to"] not in {"heat_pump_heating.u1", "heat_pump_heating.u2",
                                                    "heat_pump_cooling.u1", "heat_pump_cooling.u2",
                                                    "boiler.u1", "boiler.u2", "chiller.u1", "chiller.u2"}]
    capacity = "heat_pump_available_capacity_kw"
    graph["inputs"].append(port(capacity, unit="kW", label="Heat Pump Thermal Capacity at Conditions",
                                brick="Point", description="Capacity at outdoor conditions before the separate equipment-availability status is applied"))
    plant["inputs"].append(port(capacity, unit="kW", label="Heat Pump Thermal Capacity at Conditions", brick="Point"))
    graph["connections"].append({"from": capacity, "to": f"plant.{capacity}"})
    plant["outputs"].append(port("heat_pump_requested_mode", "Integer", "count",
                                  "Heat Pump Requested Mode", "Point",
                                  "0 off, 1 heating, 2 cooling, 3 simultaneous heating and cooling"))
    graph["outputs"].append(port("plant_heat_pump_requested_mode", "Integer", "count",
                                  "Plant Heat Pump Requested Mode", "Point"))
    graph["connections"].append({"from": "plant.heat_pump_requested_mode", "to": "plant_heat_pump_requested_mode"})
    plant["blocks"].extend([
        switch("heat_pump_hot_eligible", "kW", "Offer hot-water demand to dispatch only after measured hot-water flow is proven."),
        switch("heat_pump_chilled_eligible", "kW", "Offer chilled-water demand to dispatch only after measured chilled-water flow is proven."),
        {"name": "heat_pump_utilization_limit", "block_type": "MultiplyByParameter", "category": "elementary",
         "inputs": [port("u", unit="kW")], "outputs": [port("y", unit="kW")],
         "parameters": [{"name": "k", "type": "Real", "value": 1.0}],
         "notes": "Editable fraction of available equipment capacity offered to the heat pump. Legacy dispatch tuning migrates here; it no longer splits every demand by a fixed share."},
        constant("heat_pump_zero_capacity", 0.0, "kW", "Do not offer negative heat-pump capacity."),
        binary("heat_pump_capacity_nonnegative", "Max", "Real", "Real", "Clamp edited utilization at zero."),
        binary("heat_pump_capacity_limited", "Min", "Real", "Real", "Never offer more than physical available capacity."),
        binary("heat_pump_total_load", "Add", "Real", "Real", "Total hot and chilled source demand."),
        constant("heat_pump_divisor_floor", 0.001, "kW", "Avoid division by zero when the plant is idle."),
        binary("heat_pump_safe_total", "Max", "Real", "Real", "Positive denominator for allocation."),
        binary("heat_pump_capacity_ratio", "Divide", "Real", "Real", "Available capacity divided by total demand."),
        constant("heat_pump_full_fraction", 1.0, "fraction (0–1)", "Do not command above either water-loop demand."),
        binary("heat_pump_allocated_fraction", "Min", "Real", "Real", "Share of each load served by the heat pump within combined capacity."),
        binary("boiler_residual", "Subtract", "Real", "Real", "Boiler serves the hot-water load left after heat-pump allocation."),
        binary("chiller_residual", "Subtract", "Real", "Real", "Chiller serves the chilled-water load left after heat-pump allocation."),
        constant("heat_pump_mode_threshold", 0.5, "kW", "Ignore negligible loads when identifying operating mode."),
        binary("heat_pump_heating_requested", "Greater", note="Heating demand is active."),
        binary("heat_pump_cooling_requested", "Greater", note="Cooling demand is active."),
        {"name": "heat_pump_heating_mode_code", "block_type": "BooleanToInteger", "category": "elementary",
         "inputs": [port("u", "Boolean")], "outputs": [port("y", "Integer", "count")],
         "notes": "One when heating is requested."},
        {"name": "heat_pump_cooling_mode_code", "block_type": "BooleanToInteger", "category": "elementary",
         "inputs": [port("u", "Boolean")], "outputs": [port("y", "Integer", "count")],
         "notes": "One when cooling is requested."},
        {"name": "heat_pump_mode_cooling_weight", "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant", "category": "elementary",
         "inputs": [], "outputs": [port("y", "Integer", "count")],
         "parameters": [{"name": "k", "type": "Integer", "value": 2}],
         "notes": "Cooling contributes two to the requested-mode code."},
        {"name": "heat_pump_cooling_mode_weight", "block_type": "Buildings.Controls.OBC.CDL.Integers.Multiply", "category": "elementary",
         "inputs": [port("u1", "Integer", "count"), port("u2", "Integer", "count")],
         "outputs": [port("y", "Integer", "count")],
         "notes": "Encode cooling as mode two."},
        binary("heat_pump_mode_sum", "Add", "Integer", "Integer", "Mode 0 off, 1 heating, 2 cooling, or 3 simultaneous service."),
    ])
    plant["connections"].extend({"from": source, "to": target} for source, target in [
        ("hot_water_source_nonnegative.y", "heat_pump_hot_eligible.u1"),
        ("hot_source_zero.y", "heat_pump_hot_eligible.u2"),
        ("hot_source_proven.y", "heat_pump_hot_eligible.u3"),
        ("chilled_water_source_nonnegative.y", "heat_pump_chilled_eligible.u1"),
        ("chilled_source_zero.y", "heat_pump_chilled_eligible.u2"),
        ("chilled_source_proven.y", "heat_pump_chilled_eligible.u3"),
        (capacity, "heat_pump_utilization_limit.u"),
        ("heat_pump_utilization_limit.y", "heat_pump_capacity_nonnegative.u1"),
        ("heat_pump_zero_capacity.y", "heat_pump_capacity_nonnegative.u2"),
        ("heat_pump_capacity_nonnegative.y", "heat_pump_capacity_limited.u1"),
        (capacity, "heat_pump_capacity_limited.u2"),
        ("heat_pump_hot_eligible.y", "heat_pump_total_load.u1"),
        ("heat_pump_chilled_eligible.y", "heat_pump_total_load.u2"),
        ("heat_pump_total_load.y", "heat_pump_safe_total.u1"),
        ("heat_pump_divisor_floor.y", "heat_pump_safe_total.u2"),
        ("heat_pump_capacity_limited.y", "heat_pump_capacity_ratio.u1"),
        ("heat_pump_safe_total.y", "heat_pump_capacity_ratio.u2"),
        ("heat_pump_capacity_ratio.y", "heat_pump_allocated_fraction.u1"),
        ("heat_pump_full_fraction.y", "heat_pump_allocated_fraction.u2"),
        ("heat_pump_hot_eligible.y", "heat_pump_heating.u1"),
        ("heat_pump_chilled_eligible.y", "heat_pump_cooling.u1"),
        ("heat_pump_allocated_fraction.y", "heat_pump_heating.u2"),
        ("heat_pump_allocated_fraction.y", "heat_pump_cooling.u2"),
        ("heat_pump_hot_eligible.y", "boiler_residual.u1"),
        ("heat_pump_heating.y", "boiler_residual.u2"),
        ("heat_pump_chilled_eligible.y", "chiller_residual.u1"),
        ("heat_pump_cooling.y", "chiller_residual.u2"),
        ("boiler_residual.y", "boiler.u1"),
        ("chiller_residual.y", "chiller.u1"),
        ("one.y", "boiler.u2"),
        ("one.y", "chiller.u2"),
        ("heat_pump_hot_eligible.y", "heat_pump_heating_requested.u1"),
        ("heat_pump_chilled_eligible.y", "heat_pump_cooling_requested.u1"),
        ("heat_pump_mode_threshold.y", "heat_pump_heating_requested.u2"),
        ("heat_pump_mode_threshold.y", "heat_pump_cooling_requested.u2"),
        ("heat_pump_heating_requested.y", "heat_pump_heating_mode_code.u"),
        ("heat_pump_cooling_requested.y", "heat_pump_cooling_mode_code.u"),
        ("heat_pump_cooling_mode_code.y", "heat_pump_cooling_mode_weight.u1"),
        ("heat_pump_mode_cooling_weight.y", "heat_pump_cooling_mode_weight.u2"),
        ("heat_pump_heating_mode_code.y", "heat_pump_mode_sum.u1"),
        ("heat_pump_cooling_mode_weight.y", "heat_pump_mode_sum.u2"),
        ("heat_pump_mode_sum.y", "heat_pump_requested_mode"),
    ])
    plant["notes"] += " Equipment-specific CDL allocation shares available heat-pump capacity across hot and chilled demand and commands boiler/chiller for the residual. The physical simulator approximates simultaneous compressor energy without a calibrated refrigerant model."


add_capacity_aware_heat_pump_dispatch()

def add_equipment_availability_control():
    """Let CDL qualify source commands with raw equipment availability."""
    plant = next(block for block in graph["blocks"] if block["name"] == "plant")
    for equipment in ("heat_pump", "boiler", "chiller"):
        name = f"{equipment}_available"
        label = f"{equipment.replace('_', ' ').title()} Available Status"
        graph["inputs"].append(port(name, "Boolean", "unitless", label, "Status",
                                    "Raw equipment availability from the physical model; false during a test outage"))
        plant["inputs"].append(port(name, "Boolean", "unitless", label, "Status"))
        graph["connections"].append({"from": name, "to": f"plant.{name}"})
    plant["blocks"].extend([
        switch("heat_pump_availability_gate", "kW", "Offer capacity only while the heat pump reports available; boiler and chiller residuals remain CDL decisions."),
        switch("boiler_availability_gate", "kW", "Command the boiler only while its raw availability status is true."),
        switch("chiller_availability_gate", "kW", "Command the chiller only while its raw availability status is true."),
    ])
    replaced = {
        ("heat_pump_capacity_limited.y", "heat_pump_capacity_ratio.u1"),
        ("boiler_flow_gate.y", "boiler_kw"), ("chiller_flow_gate.y", "chiller_kw"),
        ("boiler_flow_gate.y", "hot_stage_backup_active.u1"),
        ("chiller_flow_gate.y", "chilled_stage_backup_active.u1"),
        ("heat_pump_hot_eligible.y", "heat_pump_heating_requested.u1"),
        ("heat_pump_chilled_eligible.y", "heat_pump_cooling_requested.u1"),
    }
    plant["connections"] = [edge for edge in plant["connections"]
                            if (edge["from"], edge["to"]) not in replaced]
    plant["connections"].extend({"from": source, "to": target} for source, target in [
        ("heat_pump_capacity_limited.y", "heat_pump_availability_gate.u1"),
        ("heat_pump_zero_capacity.y", "heat_pump_availability_gate.u2"),
        ("heat_pump_available", "heat_pump_availability_gate.u3"),
        ("heat_pump_availability_gate.y", "heat_pump_capacity_ratio.u1"),
        ("boiler_flow_gate.y", "boiler_availability_gate.u1"),
        ("hot_source_zero.y", "boiler_availability_gate.u2"),
        ("boiler_available", "boiler_availability_gate.u3"),
        ("boiler_availability_gate.y", "boiler_kw"),
        ("chiller_flow_gate.y", "chiller_availability_gate.u1"),
        ("chilled_source_zero.y", "chiller_availability_gate.u2"),
        ("chiller_available", "chiller_availability_gate.u3"),
        ("chiller_availability_gate.y", "chiller_kw"),
        ("boiler_availability_gate.y", "hot_stage_backup_active.u1"),
        ("chiller_availability_gate.y", "chilled_stage_backup_active.u1"),
        ("heat_pump_heating.y", "heat_pump_heating_requested.u1"),
        ("heat_pump_cooling.y", "heat_pump_cooling_requested.u1"),
    ])
    plant["notes"] += " Raw equipment-availability inputs gate heat-pump capacity and backup source commands in CDL."


def add_physical_source_stage_feedback():
    """Use raw running feedback to qualify the graph's sampled reset holds."""
    plant = next(block for block in graph["blocks"] if block["name"] == "plant")
    for equipment in ("heat_pump", "boiler", "chiller"):
        name = f"{equipment}_running"
        label = f"{equipment.replace('_', ' ').title()} Running Status"
        graph["inputs"].append(port(name, "Boolean", "unitless", label, "Status",
                                    "Physical source running feedback from the previous simulation step"))
        plant["inputs"].append(port(name, "Boolean", "unitless", label, "Status"))
        graph["connections"].append({"from": name, "to": f"plant.{name}"})
    for loop, backup in (("hot", "boiler"), ("chilled", "chiller")):
        blocks = plant["blocks"]
        edges = plant["connections"]
        blocks.extend([
            binary(f"{loop}_stage_hp_confirmed", "And", "Boolean", "Boolean",
                   "The heat-pump stage is commanded and physically running."),
            binary(f"{loop}_stage_backup_confirmed", "And", "Boolean", "Boolean",
                   "The backup source stage is commanded and physically running."),
            {"name": f"{loop}_stage_hp_not_requested", "block_type": "Not", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "notes": "An unrequested heat-pump stage needs no startup proof."},
            {"name": f"{loop}_stage_backup_not_requested", "block_type": "Not", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "notes": "An unrequested backup stage needs no startup proof."},
            binary(f"{loop}_stage_hp_ready", "Or", "Boolean", "Boolean",
                   "Heat-pump stage is either unrequested or physically running."),
            binary(f"{loop}_stage_backup_ready", "Or", "Boolean", "Boolean",
                   "Backup stage is either unrequested or physically running."),
            binary(f"{loop}_stage_sources_ready", "And", "Boolean", "Boolean",
                   "Both commanded source stages have physical running proof."),
            {"name": f"{loop}_stage_ready_integer", "block_type": "BooleanToInteger", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Integer", "count")],
             "notes": "Encode physical stage readiness as zero or one for the sampled delay."},
            {"name": f"{loop}_stage_ready_real", "block_type": "IntegerToReal", "category": "elementary",
             "inputs": [port("u", "Integer", "count")], "outputs": [port("y", "Real", "count")],
             "notes": "Provide a real-valued readiness signal to the sampled delay."},
            {"name": f"{loop}_stage_ready_observed", "block_type": "UnitDelay", "category": "elementary",
             "inputs": [port("u", unit="count")], "outputs": [port("y", unit="count")],
             "parameters": [{"name": "samplePeriod", "value": 60.0, "unit": "s"},
                            {"name": "y_start", "value": 0.0, "unit": "count"}],
             "notes": "Observe prior source readiness so water reset cannot feed directly back into source commands."},
            constant(f"{loop}_stage_ready_threshold", 0.5, "count", "Convert the delayed readiness code back to a Boolean."),
            binary(f"{loop}_stage_ready_confirmed", "Greater", "Real", "Boolean",
                   "The prior sampled source readiness code was true."),
            binary(f"{loop}_stage_ready_and_flow", "And", "Boolean", "Boolean",
                   "Allow the sampled reset hold timer only after stage and water-flow proof."),
        ])
        replaced = {f"{loop}_stage_hp_select.u3", f"{loop}_stage_backup_select.u3",
                    f"{loop}_stage_hold_delay.u"}
        edges[:] = [edge for edge in edges if edge["to"] not in replaced]
        edges.extend({"from": source, "to": target} for source, target in [
            (f"{loop}_stage_hp_active.y", f"{loop}_stage_hp_confirmed.u1"),
            ("heat_pump_running", f"{loop}_stage_hp_confirmed.u2"),
            (f"{loop}_stage_hp_confirmed.y", f"{loop}_stage_hp_select.u3"),
            (f"{loop}_stage_backup_active.y", f"{loop}_stage_backup_confirmed.u1"),
            (f"{backup}_running", f"{loop}_stage_backup_confirmed.u2"),
            (f"{loop}_stage_backup_confirmed.y", f"{loop}_stage_backup_select.u3"),
            (f"{loop}_stage_hp_active.y", f"{loop}_stage_hp_not_requested.u"),
            (f"{loop}_stage_backup_active.y", f"{loop}_stage_backup_not_requested.u"),
            (f"{loop}_stage_hp_not_requested.y", f"{loop}_stage_hp_ready.u1"),
            ("heat_pump_running", f"{loop}_stage_hp_ready.u2"),
            (f"{loop}_stage_backup_not_requested.y", f"{loop}_stage_backup_ready.u1"),
            (f"{backup}_running", f"{loop}_stage_backup_ready.u2"),
            (f"{loop}_stage_hp_ready.y", f"{loop}_stage_sources_ready.u1"),
            (f"{loop}_stage_backup_ready.y", f"{loop}_stage_sources_ready.u2"),
            (f"{loop}_stage_sources_ready.y", f"{loop}_stage_ready_integer.u"),
            (f"{loop}_stage_ready_integer.y", f"{loop}_stage_ready_real.u"),
            (f"{loop}_stage_ready_real.y", f"{loop}_stage_ready_observed.u"),
            (f"{loop}_stage_ready_observed.y", f"{loop}_stage_ready_confirmed.u1"),
            (f"{loop}_stage_ready_threshold.y", f"{loop}_stage_ready_confirmed.u2"),
            (f"{loop}_stage_ready_confirmed.y", f"{loop}_stage_ready_and_flow.u1"),
            (f"{loop}_stage_stable_and_proven.y", f"{loop}_stage_ready_and_flow.u2"),
            (f"{loop}_stage_ready_and_flow.y", f"{loop}_stage_hold_delay.u"),
        ])
        for name in (f"{loop}_stage_observed", f"{loop}_stage_previous"):
            next(block for block in blocks if block["name"] == name)["notes"] = (
                "Sample the previously confirmed physical source-stage combination for reset hold timing.")
    plant["notes"] += " Sampled water-reset holds now require raw source running status as well as water-flow proof."


def add_heat_pump_start_fault_control():
    """Latch a failed start in CDL and let residual dispatch move load to backups."""
    plant = next(block for block in graph["blocks"] if block["name"] == "plant")
    reset = "heat_pump_fault_reset"
    graph["inputs"].append(port(reset, "Boolean", "unitless", "Heat Pump Fault Reset", "Status",
                                "Raw operator reset pulse; the CDL latch decides fault recovery"))
    plant["inputs"].append(port(reset, "Boolean", "unitless", "Heat Pump Fault Reset", "Status"))
    graph["connections"].append({"from": reset, "to": f"plant.{reset}"})
    plant["outputs"].append(port("heat_pump_start_fault", "Boolean", "unitless",
                                  "Heat Pump Start Fault", "Alarm"))
    graph["outputs"].append(port("plant_heat_pump_start_fault", "Boolean", "unitless",
                                  "Plant Heat Pump Start Fault", "Alarm"))
    graph["connections"].append({"from": "plant.heat_pump_start_fault", "to": "plant_heat_pump_start_fault"})
    plant["blocks"].extend([
        binary("heat_pump_start_request", "Add", "Real", "Real", "Combined heat-pump source command."),
        {"name": "heat_pump_start_request_observed", "block_type": "UnitDelay", "category": "elementary",
         "inputs": [port("u", unit="kW")], "outputs": [port("y", unit="kW")],
         "parameters": [{"name": "samplePeriod", "value": 60.0, "unit": "s"},
                        {"name": "y_start", "value": 0.0, "unit": "kW"}],
         "notes": "Use the prior command to break the fault-to-dispatch feedback loop."},
        constant("heat_pump_start_threshold", 0.01, "kW", "Minimum command treated as a start request."),
        binary("heat_pump_start_requested", "Greater", "Real", "Boolean", "Detect a prior nonzero command."),
        {"name": "heat_pump_not_running", "block_type": "Not", "category": "elementary",
         "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
         "notes": "Physical running proof is absent."},
        binary("heat_pump_start_unproven", "And", "Boolean", "Boolean", "Commanded start lacks running proof."),
        binary("heat_pump_start_available", "And", "Boolean", "Boolean", "Only diagnose an available source."),
        {"name": "heat_pump_start_timeout", "block_type": "TrueDelay", "category": "elementary",
         "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
         "parameters": [{"name": "delayTime", "value": 300.0, "unit": "s"},
                        {"name": "delayOnInit", "type": "Boolean", "value": True}],
         "notes": "Editable continuous failure-to-prove timeout."},
        {"name": "heat_pump_start_fault_latch", "block_type": "Latch", "category": "elementary",
         "inputs": [port("u", "Boolean"), port("clr", "Boolean")],
         "outputs": [port("y", "Boolean")],
         "notes": "Keep the start fault active until an operator reset pulse."},
        {"name": "heat_pump_fault_clear", "block_type": "Not", "category": "elementary",
         "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
         "notes": "Disallow heat-pump dispatch while the start fault is latched."},
        binary("heat_pump_can_dispatch", "And", "Boolean", "Boolean", "Require availability and no start fault."),
    ])
    plant["connections"] = [edge for edge in plant["connections"]
                            if edge["to"] != "heat_pump_availability_gate.u3"]
    plant["connections"].extend({"from": source, "to": target} for source, target in [
        ("heat_pump_heating.y", "heat_pump_start_request.u1"),
        ("heat_pump_cooling.y", "heat_pump_start_request.u2"),
        ("heat_pump_start_request.y", "heat_pump_start_request_observed.u"),
        ("heat_pump_start_request_observed.y", "heat_pump_start_requested.u1"),
        ("heat_pump_start_threshold.y", "heat_pump_start_requested.u2"),
        ("heat_pump_running", "heat_pump_not_running.u"),
        ("heat_pump_start_requested.y", "heat_pump_start_unproven.u1"),
        ("heat_pump_not_running.y", "heat_pump_start_unproven.u2"),
        ("heat_pump_start_unproven.y", "heat_pump_start_available.u1"),
        ("heat_pump_available", "heat_pump_start_available.u2"),
        ("heat_pump_start_available.y", "heat_pump_start_timeout.u"),
        ("heat_pump_start_timeout.y", "heat_pump_start_fault_latch.u"),
        (reset, "heat_pump_start_fault_latch.clr"),
        ("heat_pump_start_fault_latch.y", "heat_pump_start_fault"),
        ("heat_pump_start_fault_latch.y", "heat_pump_fault_clear.u"),
        ("heat_pump_fault_clear.y", "heat_pump_can_dispatch.u1"),
        ("heat_pump_available", "heat_pump_can_dispatch.u2"),
        ("heat_pump_can_dispatch.y", "heat_pump_availability_gate.u3"),
    ])
    plant["notes"] += " An editable timed CDL fault latch removes heat-pump capacity after a failed start; operator reset permits another attempt."


def add_backup_start_fault_control(equipment):
    """Diagnose a backup source's failed start entirely in editable CDL."""
    plant = next(block for block in graph["blocks"] if block["name"] == "plant")
    title = equipment.title()
    reset = f"{equipment}_fault_reset"
    fault = f"{equipment}_start_fault"
    graph["inputs"].append(port(reset, "Boolean", "unitless", f"{title} Fault Reset", "Status",
                                "Raw operator reset pulse; the CDL latch decides fault recovery"))
    plant["inputs"].append(port(reset, "Boolean", "unitless", f"{title} Fault Reset", "Status"))
    graph["connections"].append({"from": reset, "to": f"plant.{reset}"})
    plant["outputs"].append(port(fault, "Boolean", "unitless", f"{title} Start Fault", "Alarm"))
    graph["outputs"].append(port(f"plant_{fault}", "Boolean", "unitless",
                                  f"Plant {title} Start Fault", "Alarm"))
    graph["connections"].append({"from": f"plant.{fault}", "to": f"plant_{fault}"})
    plant["blocks"].extend([
        {"name": f"{equipment}_start_request_observed", "block_type": "UnitDelay", "category": "elementary",
         "inputs": [port("u", unit="kW")], "outputs": [port("y", unit="kW")],
         "parameters": [{"name": "samplePeriod", "value": 60.0, "unit": "s"},
                        {"name": "y_start", "value": 0.0, "unit": "kW"}],
         "notes": "Sample the prior source command so fault feedback does not form a combinational cycle."},
        constant(f"{equipment}_start_threshold", 0.01, "kW", "Minimum command treated as a start request."),
        binary(f"{equipment}_start_requested", "Greater", "Real", "Boolean", "Detect a prior nonzero command."),
        {"name": f"{equipment}_not_running", "block_type": "Not", "category": "elementary",
         "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
         "notes": "Physical running proof is absent."},
        binary(f"{equipment}_start_unproven", "And", "Boolean", "Boolean", "Commanded start lacks running proof."),
        binary(f"{equipment}_start_available", "And", "Boolean", "Boolean", "Only diagnose an available source."),
        {"name": f"{equipment}_start_timeout", "block_type": "TrueDelay", "category": "elementary",
         "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
         "parameters": [{"name": "delayTime", "value": 300.0, "unit": "s"},
                        {"name": "delayOnInit", "type": "Boolean", "value": True}],
         "notes": "Editable continuous failure-to-prove timeout."},
        {"name": f"{equipment}_start_fault_latch", "block_type": "Latch", "category": "elementary",
         "inputs": [port("u", "Boolean"), port("clr", "Boolean")],
         "outputs": [port("y", "Boolean")],
         "notes": "Keep the start fault active until an operator reset pulse."},
        {"name": f"{equipment}_fault_clear", "block_type": "Not", "category": "elementary",
         "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
         "notes": "Disallow the source command while the start fault is latched."},
        binary(f"{equipment}_can_dispatch", "And", "Boolean", "Boolean", "Require availability and no start fault."),
    ])
    gate = f"{equipment}_availability_gate"
    plant["connections"] = [edge for edge in plant["connections"] if edge["to"] != f"{gate}.u3"]
    plant["connections"].extend({"from": source, "to": target} for source, target in [
        (f"{gate}.y", f"{equipment}_start_request_observed.u"),
        (f"{equipment}_start_request_observed.y", f"{equipment}_start_requested.u1"),
        (f"{equipment}_start_threshold.y", f"{equipment}_start_requested.u2"),
        (f"{equipment}_running", f"{equipment}_not_running.u"),
        (f"{equipment}_start_requested.y", f"{equipment}_start_unproven.u1"),
        (f"{equipment}_not_running.y", f"{equipment}_start_unproven.u2"),
        (f"{equipment}_start_unproven.y", f"{equipment}_start_available.u1"),
        (f"{equipment}_available", f"{equipment}_start_available.u2"),
        (f"{equipment}_start_available.y", f"{equipment}_start_timeout.u"),
        (f"{equipment}_start_timeout.y", f"{equipment}_start_fault_latch.u"),
        (reset, f"{equipment}_start_fault_latch.clr"),
        (f"{equipment}_start_fault_latch.y", fault),
        (f"{equipment}_start_fault_latch.y", f"{equipment}_fault_clear.u"),
        (f"{equipment}_fault_clear.y", f"{equipment}_can_dispatch.u1"),
        (f"{equipment}_available", f"{equipment}_can_dispatch.u2"),
        (f"{equipment}_can_dispatch.y", f"{gate}.u3"),
    ])
    plant["notes"] += f" An editable timed CDL fault latch inhibits {equipment} after a failed start until operator reset."


graph["inputs"].append(port(
    "schedule_occupied", "Boolean", "unitless", "Occupied schedule",
    "Point", "Building occupied schedule from the weather year's 15-minute calendar",
))
graph["inputs"].append(port(
    "seconds_until_occupied", unit="s", label="Time Until Next Occupied Schedule",
    brick="Point", description="Calendar time until the next scheduled occupied sample",
))
graph["outputs"].append(port(
    "preconditioning_mode", "Boolean", "unitless", "Preconditioning Mode Command",
    "Point", "Computed by the editable schedule lookahead CDL sequence",
))
graph["blocks"].extend([
    constant("preconditioning_horizon", 21601.0, "s", "Start conditioning within six hours of scheduled occupancy."),
    binary("within_preconditioning_horizon", "Less", note="Compare calendar time until occupancy with the configured horizon."),
    {"name": "schedule_unoccupied", "block_type": "Not", "category": "elementary",
     "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
     "notes": "Preconditioning can run only before the scheduled occupied period."},
    binary("preconditioning_enabled", "And", "Boolean", note="Editable preconditioning mode decision."),
])
graph["connections"].extend({"from": source, "to": target} for source, target in [
    ("seconds_until_occupied", "within_preconditioning_horizon.u1"),
    ("preconditioning_horizon.y", "within_preconditioning_horizon.u2"),
    ("schedule_occupied", "schedule_unoccupied.u"),
    ("within_preconditioning_horizon.y", "preconditioning_enabled.u1"),
    ("schedule_unoccupied.y", "preconditioning_enabled.u2"),
    ("preconditioning_enabled.y", "preconditioning_mode"),
])

for index in range(1, 11):
    name = f"zone_{index:02}"
    graph["inputs"].append(port(
        f"{name}_occupied", "Boolean", "unitless", f"Zone {index} Occupancy Sensor",
        "Occupancy_Sensor", "Raw synthetic zone presence, sampled every 15 minutes",
    ))
    graph["inputs"].extend([
        port(f"{name}_measured_airflow", unit="normalized airflow", label=f"Zone {index} Primary Air Flow Sensor", brick="Supply_Air_Flow_Sensor", description="Delivered primary air from the previous physical-model step"),
        port(f"{name}_discharge_temp", unit="°C", label=f"Zone {index} Discharge Air Temperature Sensor", brick="Discharge_Air_Temperature_Sensor", description="Temperature after terminal reheat from the previous physical-model step"),
    ])
    if graph.get("brick_model"):
        discharge_sensor = f"{name}_discharge_temp"
        graph["brick_model"]["entities"].append({
            "id": discharge_sensor, "class": "brick:Discharge_Air_Temperature_Sensor",
            "label": f"Zone {index:02d} discharge air temperature", "connector": discharge_sensor,
        })
        graph["brick_model"]["relationships"].append({
            "subject": f"vav_{index:02d}_equipment", "predicate": "brick:hasPoint",
            "object": discharge_sensor,
        })
    graph["outputs"].append(port(
        f"{name}_occupancy_confirmed", "Boolean", "unitless",
        f"Zone {index} Confirmed Occupancy", "Point",
        "Result of the editable CDL one-minute arrival and five-minute vacancy sequence",
    ))
    zone = next(block for block in graph["blocks"] if block["name"] == name)
    zone["description"] = "G36-style zone setpoint and minimum-airflow reset with local occupancy sensing."
    zone["notes"] = (
        "The scheduled mode uses occupied 21/24 °C setpoints. Five minutes without a "
        "person widens both setpoints by 0.5 K and reduces ventilation minimums; one "
        "minute of presence restores them. Outside the occupied schedule, shared group "
        "setback and setup modes enable editable 18/32 °C targets; otherwise heating "
        "and cooling are off and ventilation minimum is zero. Before occupancy, "
        "warm-up or cooldown restores occupied temperature setpoints but keeps the reduced "
        "ventilation minimum. Heating mode raises airflow above minimum only after the discharge is warm, while preconditioning can use the heating maximum. CDL TrueDelay "
        "and Latch blocks confirm arrival and vacancy from the raw occupancy sensor. Measured "
        "airflow and discharge temperature are fed back from the physical model. A CDL "
        "discharge-temperature reset and valve PI loop command reheat, and five-minute low-flow and "
        "high-discharge alarms report persistent faults."
    )
    zone["inputs"].extend([
        port("scheduled_occupied", "Boolean", "unitless", "Occupied schedule", "Point"),
        port("preconditioning_mode", "Boolean", "unitless", "Preconditioning mode", "Point"),
        port("zone_presence_raw", "Boolean", "unitless", "Raw Zone Occupancy Sensor", "Occupancy_Sensor"),
        port("measured_airflow", unit="normalized airflow", label="Primary Air Flow Sensor", brick="Supply_Air_Flow_Sensor"),
        port("discharge_temp", unit="°C", label="Discharge Air Temperature Sensor", brick="Discharge_Air_Temperature_Sensor"),
    ])
    zone["outputs"].append(port(
        "occupancy_confirmed", "Boolean", "unitless", "Confirmed Occupancy", "Point",
    ))
    zone["outputs"].extend([
        port("low_airflow_alarm", "Boolean", "unitless", "Low Primary Air Flow Alarm", "Point"),
        port("high_discharge_alarm", "Boolean", "unitless", "High Discharge Temperature Alarm", "Point"),
        port("pressure_reset_request", "Integer", "count", "Duct Pressure Reset Request", "Point"),
    ])
    graph["outputs"].extend([
        port(f"{name}_low_airflow_alarm", "Boolean", "unitless", f"Zone {index} Low Primary Air Flow Alarm", "Point"),
        port(f"{name}_high_discharge_alarm", "Boolean", "unitless", f"Zone {index} High Discharge Temperature Alarm", "Point"),
        port(f"{name}_pressure_reset_request", "Integer", "count", f"Zone {index} Duct Pressure Reset Request", "Point"),
    ])
    graph["connections"].extend([
        {"from": "schedule_occupied", "to": f"{name}.scheduled_occupied"},
        {"from": "preconditioning_enabled.y", "to": f"{name}.preconditioning_mode"},
        {"from": f"{name}_occupied", "to": f"{name}.zone_presence_raw"},
        {"from": f"{name}.occupancy_confirmed", "to": f"{name}_occupancy_confirmed"},
        {"from": f"{name}_measured_airflow", "to": f"{name}.measured_airflow"},
        {"from": f"{name}_discharge_temp", "to": f"{name}.discharge_temp"},
        {"from": f"{name}.low_airflow_alarm", "to": f"{name}_low_airflow_alarm"},
        {"from": f"{name}.high_discharge_alarm", "to": f"{name}_high_discharge_alarm"},
        {"from": f"{name}.pressure_reset_request", "to": f"{name}_pressure_reset_request"},
    ])
    blocks = zone["blocks"]
    by_name = {block["name"]: block for block in blocks}
    by_name["cool_setpoint"]["parameters"][0]["value"] = 24.0
    by_name["cool_setpoint"]["notes"] = "G36 occupied cooling setpoint: 24 °C."
    by_name["heat_setpoint"]["notes"] = "G36 occupied heating setpoint: 21 °C."
    by_name["damper_limit"]["parameters"] = [
        {**parameter, "value": 0.0} if parameter["name"] == "uMin" else parameter
        for parameter in by_name["damper_limit"]["parameters"]
    ]
    blocks.extend([
        constant("maximum_discharge_temp", 35.0, "°C", "Editable terminal discharge temperature upper target; physical coil safety remains at 40 °C."),
        {"name": "discharge_headroom", "block_type": "Subtract", "category": "elementary", "inputs": [port("u1", unit="°C"), port("u2", unit="°C")], "outputs": [port("y", unit="K")], "notes": "Temperature rise allowed above measured AHU supply air."},
        {"name": "reheat_capacity_per_flow", "block_type": "MultiplyByParameter", "category": "elementary", "inputs": [port("u", unit="K")], "outputs": [port("y", unit="fraction (0–1) / normalized airflow")], "parameters": [{"name": "k", "type": "Real", "value": 0.15, "unit": "normalized airflow⁻¹ K⁻¹"}], "notes": "Converts temperature headroom to valve capacity: 1.2 kW per airflow-K divided by the 8 kW reheat coil."},
        binary("reheat_flow_cap", "Multiply", "Real", "Real", "Scale valve cap by measured primary airflow."),
        {"name": "reheat_cap_limit", "block_type": "Limiter", "category": "elementary", "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", unit="fraction (0–1)")], "parameters": [{"name": "uMin", "value": 0.0}, {"name": "uMax", "value": 1.0}], "notes": "Keep the discharge-temperature-derived valve cap between closed and open."},
        binary("reheat_capped", "Min", "Real", "Real", "Limit zone heating command with the discharge temperature cap."),
        constant("low_flow_fraction", 0.5, "fraction (0–1)", "Alarm when measured flow stays below half of its request."),
        binary("low_flow_threshold", "Multiply", "Real", "Real", "Half of requested primary airflow."),
        binary("low_flow_detected", "Less", note="Measured primary airflow is below half the request."),
        constant("flow_alarm_enable_threshold", 0.1, "normalized airflow", "Avoid alarm when the VAV is not requesting air."),
        binary("flow_requested", "Greater", note="Only monitor low flow while the box requests air."),
        binary("low_flow_condition", "And", "Boolean", note="Request exists and delivered primary flow is low."),
        {"name": "low_flow_delay", "block_type": "Buildings.Controls.OBC.CDL.Logical.TrueDelay", "category": "elementary", "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")], "parameters": [{"name": "delayTime", "value": 300.0, "unit": "s"}, {"name": "delayOnInit", "value": True, "type": "Boolean"}], "notes": "Require five continuous minutes of deficient primary flow before alarm."},
        binary("high_discharge_detected", "Greater", note="Measured discharge temperature exceeds the editable upper target."),
        {"name": "high_discharge_delay", "block_type": "Buildings.Controls.OBC.CDL.Logical.TrueDelay", "category": "elementary", "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")], "parameters": [{"name": "delayTime", "value": 300.0, "unit": "s"}, {"name": "delayOnInit", "value": True, "type": "Boolean"}], "notes": "Require five continuous minutes above the discharge upper target before alarm."},
        constant("standby_cool", 24.5, "°C", "Occupied standby cooling setpoint after five minutes vacant."),
        constant("standby_heat", 20.5, "°C", "Occupied standby heating setpoint after five minutes vacant."),
        constant("unoccupied_cool", 32.0, "°C", "G36 unoccupied cooling setback setpoint."),
        constant("unoccupied_heat", 18.0, "°C", "Editable unoccupied heating setback; the three-hour optimal-start cap can recover from this teaching target."),
        constant("standby_damper", 0.0, "fraction (0–1)", "No occupied-standby ventilation minimum while the zone is vacant."),
        constant("unoccupied_damper", 0.0, "fraction (0–1)", "No ventilation minimum outside occupied schedule."),
        constant("standby_airflow", 0.0, "normalized airflow", "No occupied-standby airflow minimum while the zone is vacant."),
        constant("unoccupied_airflow", 0.0, "normalized airflow", "No minimum airflow outside occupied schedule."),
        {"name": "presence_delay", "block_type": "Buildings.Controls.OBC.CDL.Logical.TrueDelay", "category": "elementary",
         "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
         "parameters": [{"name": "delayTime", "value": 60.0, "unit": "s"}, {"name": "delayOnInit", "value": True, "type": "Boolean"}],
         "notes": "Require one continuous minute of presence before setting occupied."},
        {"name": "presence_inverse", "block_type": "Buildings.Controls.OBC.CDL.Logical.Not", "category": "elementary",
         "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
         "notes": "True when the raw occupancy sensor reports vacant."},
        {"name": "vacancy_delay", "block_type": "Buildings.Controls.OBC.CDL.Logical.TrueDelay", "category": "elementary",
         "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
         "parameters": [{"name": "delayTime", "value": 300.0, "unit": "s"}, {"name": "delayOnInit", "value": True, "type": "Boolean"}],
         "notes": "Require five continuous minutes vacant before clearing occupied."},
        {"name": "occupancy_latch", "block_type": "Buildings.Controls.OBC.CDL.Logical.Latch", "category": "elementary",
         "inputs": [port("u", "Boolean"), port("clr", "Boolean")], "outputs": [port("y", "Boolean")],
         "notes": "Hold confirmed occupancy until the vacancy delay clears it."},
    ])
    blocks.append({
        "name": "conditioning_mode", "block_type": "Or", "category": "elementary",
        "inputs": [port("u1", "Boolean"), port("u2", "Boolean")],
        "outputs": [port("y", "Boolean")],
        "notes": "Request occupied temperature setpoints during the schedule or the preconditioning window.",
    })
    edges = zone["connections"]
    edges.extend([
        {"from": "maximum_discharge_temp.y", "to": "discharge_headroom.u1"},
        {"from": "supply_air_temp", "to": "discharge_headroom.u2"},
        {"from": "discharge_headroom.y", "to": "reheat_capacity_per_flow.u"},
        {"from": "reheat_capacity_per_flow.y", "to": "reheat_flow_cap.u1"},
        {"from": "measured_airflow", "to": "reheat_flow_cap.u2"},
        {"from": "reheat_flow_cap.y", "to": "reheat_cap_limit.u"},
        {"from": "reheat_select.y", "to": "reheat_capped.u1"},
        {"from": "reheat_cap_limit.y", "to": "reheat_capped.u2"},
        {"from": "airflow_request_sum.y", "to": "low_flow_threshold.u1"},
        {"from": "low_flow_fraction.y", "to": "low_flow_threshold.u2"},
        {"from": "measured_airflow", "to": "low_flow_detected.u1"},
        {"from": "low_flow_threshold.y", "to": "low_flow_detected.u2"},
        {"from": "airflow_request_sum.y", "to": "flow_requested.u1"},
        {"from": "flow_alarm_enable_threshold.y", "to": "flow_requested.u2"},
        {"from": "low_flow_detected.y", "to": "low_flow_condition.u1"},
        {"from": "flow_requested.y", "to": "low_flow_condition.u2"},
        {"from": "low_flow_condition.y", "to": "low_flow_delay.u"},
        {"from": "low_flow_delay.y", "to": "low_airflow_alarm"},
        {"from": "discharge_temp", "to": "high_discharge_detected.u1"},
        {"from": "maximum_discharge_temp.y", "to": "high_discharge_detected.u2"},
        {"from": "high_discharge_detected.y", "to": "high_discharge_delay.u"},
        {"from": "high_discharge_delay.y", "to": "high_discharge_alarm"},
        {"from": "scheduled_occupied", "to": "conditioning_mode.u1"},
        {"from": "preconditioning_mode", "to": "conditioning_mode.u2"},
        {"from": "zone_presence_raw", "to": "presence_delay.u"},
        {"from": "zone_presence_raw", "to": "presence_inverse.u"},
        {"from": "presence_inverse.y", "to": "vacancy_delay.u"},
        {"from": "presence_delay.y", "to": "occupancy_latch.u"},
        {"from": "vacancy_delay.y", "to": "occupancy_latch.clr"},
        {"from": "occupancy_latch.y", "to": "occupancy_confirmed"},
    ])
    for edge in edges:
        if edge["from"] == "reheat_select.y" and edge["to"] == "reheat_valve_position":
            edge["from"] = "reheat_capped.y"
    for stem, unit in [
        ("cool_setpoint", "°C"), ("heat_setpoint", "°C"),
        ("minimum_damper", "fraction (0–1)"),
        ("minimum_airflow", "normalized airflow"),
    ]:
        blocks.append(switch(f"{stem}_standby", unit, "Select occupied or sensor-vacant standby value."))
        blocks.append(switch(f"{stem}_active", unit, "Select scheduled or unoccupied value."))
    rewired = {"cool_setpoint": 0, "heat_setpoint": 0, "minimum_damper": 0, "minimum_airflow": 0}
    for edge in edges:
        for stem in rewired:
            if edge.get("from") == f"{stem}.y":
                edge["from"] = f"{stem}_active.y"
                rewired[stem] += 1
    assert all(rewired.values()), (name, rewired)
    alternatives = {
        "cool_setpoint": ("standby_cool", "unoccupied_cool"),
        "heat_setpoint": ("standby_heat", "unoccupied_heat"),
        "minimum_damper": ("standby_damper", "unoccupied_damper"),
        "minimum_airflow": ("standby_airflow", "unoccupied_airflow"),
    }
    for stem, (standby, unoccupied) in alternatives.items():
        edges.extend([
            {"from": f"{stem}.y", "to": f"{stem}_standby.u1"},
            {"from": f"{standby}.y", "to": f"{stem}_standby.u2"},
            {"from": "occupancy_latch.y", "to": f"{stem}_standby.u3"},
            {"from": f"{stem}_standby.y", "to": f"{stem}_active.u1"},
            {"from": f"{unoccupied}.y", "to": f"{stem}_active.u2"},
            {"from": "conditioning_mode.y" if stem in ("cool_setpoint", "heat_setpoint") else "scheduled_occupied", "to": f"{stem}_active.u3"},
        ])
    for stem, unit in (("minimum_damper", "fraction (0–1)"), ("minimum_airflow", "normalized airflow")):
        blocks.append(switch(f"{stem}_heating", unit, "Preserve minimum discharge airflow during a heating call."))
        edges.extend([
            {"from": f"{stem}.y", "to": f"{stem}_heating.u1"},
            {"from": f"{stem}_active.y", "to": f"{stem}_heating.u2"},
            {"from": "heating_mode.y", "to": f"{stem}_heating.u3"},
        ])
        for edge in edges:
            if edge.get("from") == f"{stem}_active.y" and edge.get("to") != f"{stem}_heating.u2":
                edge["from"] = f"{stem}_heating.y"

    blocks.extend([
        {"name": "heating_damper_gain", "block_type": "MultiplyByParameter", "category": "elementary", "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", unit="fraction (0–1)")], "parameters": [{"name": "k", "value": 0.85, "type": "Real"}], "notes": "Increase primary-air damper opening as the heating PI demand rises."},
        {"name": "heating_damper_offset", "block_type": "Add", "category": "elementary", "inputs": [port("u1", unit="fraction (0–1)"), port("u2", unit="fraction (0–1)")], "outputs": [port("y", unit="fraction (0–1)")], "notes": "Add heating demand to the zone's active minimum damper position."},
        {"name": "heating_damper_limit", "block_type": "Limiter", "category": "elementary", "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", unit="fraction (0–1)")], "parameters": [{"name": "uMin", "value": 0.0}, {"name": "uMax", "value": 1.0}], "notes": "Keep heating damper command within its physical travel."},
        switch("heating_damper_select", "fraction (0–1)", "Use the heating airflow path during a heating call; otherwise retain the cooling/ventilation path."),
    ])
    edges.extend([
        {"from": "heat_pi.y", "to": "heating_damper_gain.u"},
        {"from": "minimum_damper_heating.y", "to": "heating_damper_offset.u1"},
        {"from": "heating_damper_gain.y", "to": "heating_damper_offset.u2"},
        {"from": "heating_damper_offset.y", "to": "heating_damper_limit.u"},
        {"from": "heating_damper_limit.y", "to": "heating_damper_select.u1"},
        {"from": "damper_select.y", "to": "heating_damper_select.u2"},
        {"from": "heating_mode.y", "to": "heating_damper_select.u3"},
    ])
    for edge in edges:
        if edge["from"] == "damper_select.y" and edge["to"] in ("damper_position", "flow_span.u"):
            edge["from"] = "heating_damper_select.y"

    blocks.extend([
        constant("pressure_request_damper_threshold", 0.9, "fraction (0–1)", "A nearly open VAV may request more duct pressure."),
        binary("damper_nearly_open", "Greater", note="The commanded damper exceeds 90% open."),
        {"name": "pressure_request_flow_fraction", "block_type": "MultiplyByParameter", "category": "elementary", "inputs": [port("u", unit="normalized airflow")], "outputs": [port("y", unit="normalized airflow")], "parameters": [{"name": "k", "value": 0.9, "type": "Real"}], "notes": "Require measured flow to reach at least 90% of request."},
        binary("pressure_request_low_flow", "Less", note="Measured primary flow is below 90% of request."),
        binary("pressure_request_condition", "And", "Boolean", note="A nearly open damper has an airflow deficit."),
        {"name": "pressure_request_delay", "block_type": "Buildings.Controls.OBC.CDL.Logical.TrueDelay", "category": "elementary", "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")], "parameters": [{"name": "delayTime", "value": 120.0, "unit": "s"}, {"name": "delayOnInit", "value": True, "type": "Boolean"}], "notes": "Require a persistent two-minute pressure request."},
        {"name": "pressure_request_integer", "block_type": "Buildings.Controls.OBC.CDL.Conversions.BooleanToInteger", "category": "elementary", "inputs": [port("u", "Boolean")], "outputs": [port("y", "Integer", "count")], "notes": "Convert the qualified request to integer 0 or 1 for AHU aggregation."},
    ])
    edges.extend({"from": source, "to": target} for source, target in [
        ("heating_damper_select.y", "damper_nearly_open.u1"),
        ("pressure_request_damper_threshold.y", "damper_nearly_open.u2"),
        ("airflow_request_sum.y", "pressure_request_flow_fraction.u"),
        ("measured_airflow", "pressure_request_low_flow.u1"),
        ("pressure_request_flow_fraction.y", "pressure_request_low_flow.u2"),
        ("damper_nearly_open.y", "pressure_request_condition.u1"),
        ("pressure_request_low_flow.y", "pressure_request_condition.u2"),
        ("pressure_request_condition.y", "pressure_request_delay.u"),
        ("pressure_request_delay.y", "pressure_request_integer.u"),
        ("pressure_request_integer.y", "pressure_reset_request"),
    ])

for ahu in (1, 2):
    controller = next(block for block in graph["blocks"] if block["name"] == f"ahu_{ahu}")
    fan_limit = next(block for block in controller["blocks"] if block["name"] == "fan_limit")
    for parameter in fan_limit["parameters"]:
        if parameter["name"] == "uMin":
            parameter["value"] = 0.0
    fan_limit["notes"] = "Zero minimum fan command permits shutdown when all zone airflow requests fall to zero."
    next(block for block in controller["blocks"] if block["name"] == "minimum_outdoor_air")["notes"] = "Fallback outdoor-air fraction when economizing is disabled; the measured-flow minimum loop may raise it."
    prefix = f"ahu_{ahu}"
    graph["inputs"].append(port(
        f"{prefix}_return_air_temp", unit="°C", label=f"AHU {ahu} Return Air Temperature Sensor",
        brick="Return_Air_Temperature_Sensor",
        description="Return temperature of the five zones served by this AHU",
    ))
    graph["inputs"].append(port(
        f"{prefix}_duct_pressure", unit="normalized pressure", label=f"AHU {ahu} Duct Static Pressure Sensor",
        brick="Point", description="Normalized duct pressure from the previous physical-model step",
    ))
    graph["inputs"].append(port(
        f"{prefix}_outdoor_airflow", unit="normalized airflow", label=f"AHU {ahu} Outside Air Flow Sensor",
        brick="Outside_Air_Flow_Sensor", description="Delivered outside air from the previous physical-model step",
    ))
    graph["outputs"].extend([
        port(f"{prefix}_pressure_request_count", "Integer", "count", f"AHU {ahu} Duct Pressure Reset Request Count", "Point"),
        port(f"{prefix}_duct_pressure_setpoint", unit="normalized pressure", label=f"AHU {ahu} Duct Static Pressure Setpoint", brick="Point"),
        port(f"{prefix}_minimum_outdoor_airflow", unit="normalized airflow", label=f"AHU {ahu} Minimum Outside Air Flow Setpoint", brick="Outside_Air_Flow_Setpoint"),
    ])
    graph["connections"].extend([
        {"from": f"{prefix}_return_air_temp", "to": f"{prefix}.return_air_temp"},
        {"from": f"{prefix}_duct_pressure", "to": f"{prefix}.duct_pressure"},
        {"from": f"{prefix}_outdoor_airflow", "to": f"{prefix}.outdoor_airflow"},
        {"from": f"{prefix}.pressure_request_count", "to": f"{prefix}_pressure_request_count"},
        {"from": f"{prefix}.duct_pressure_setpoint", "to": f"{prefix}_duct_pressure_setpoint"},
        {"from": f"{prefix}.minimum_outdoor_airflow", "to": f"{prefix}_minimum_outdoor_airflow"},
    ])
    controller["inputs"].append(port(
        "return_air_temp", unit="°C", label="Return Air Temperature Sensor",
        brick="Return_Air_Temperature_Sensor",
    ))
    controller["inputs"].append(port("duct_pressure", unit="normalized pressure", label="Duct Static Pressure Sensor", brick="Point"))
    controller["inputs"].append(port("outdoor_airflow", unit="normalized airflow", label="Outside Air Flow Sensor", brick="Outside_Air_Flow_Sensor"))
    controller["outputs"].extend([
        port("pressure_request_count", "Integer", "count", "Zone Duct Pressure Reset Request Count", "Point"),
        port("duct_pressure_setpoint", unit="normalized pressure", label="Duct Static Pressure Setpoint", brick="Point"),
        port("minimum_outdoor_airflow", unit="normalized airflow", label="Minimum Outside Air Flow Setpoint", brick="Outside_Air_Flow_Setpoint"),
    ])
    zones = [f"zone_{index:02d}" for index in range((ahu - 1) * 5 + 1, ahu * 5 + 1)]
    for name in zones:
        controller["inputs"].append(port(f"{name}_pressure_request", "Integer", "count", f"{name} Duct Pressure Request", "Point"))
        graph["connections"].append({"from": f"{name}.pressure_reset_request", "to": f"{prefix}.{name}_pressure_request"})
    for index in range(4):
        controller["blocks"].append({
            "name": f"request_sum_{index + 1}", "block_type": "Buildings.Controls.OBC.CDL.Integers.Add", "category": "elementary",
            "inputs": [port("u1", "Integer", "count"), port("u2", "Integer", "count")],
            "outputs": [port("y", "Integer", "count")],
            "notes": "Adds qualified integer VAV pressure requests for this AHU.",
        })
    controller["blocks"].extend([
        {"name": "ignored_request_count", "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant", "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "count")], "parameters": [{"name": "k", "type": "Integer", "value": 2}], "notes": "Ignore the first two zone requests before increasing the pressure target."},
        {"name": "request_count_real", "block_type": "Buildings.Controls.OBC.CDL.Conversions.IntegerToReal", "category": "elementary", "inputs": [port("u", "Integer", "count")], "outputs": [port("y", unit="count")], "notes": "Convert integer zone request count for the continuous reset calculation."},
        {"name": "pressure_request_sample", "block_type": "Buildings.Controls.OBC.CDL.Discrete.Sampler", "category": "elementary", "inputs": [port("u", unit="count")], "outputs": [port("y", unit="count")], "parameters": [{"name": "samplePeriod", "value": 120.0, "unit": "s"}], "notes": "Sample the total zone request count at the same 120-second clock as the duct-pressure reset."},
        {"name": "ignored_request_real", "block_type": "Buildings.Controls.OBC.CDL.Conversions.IntegerToReal", "category": "elementary", "inputs": [port("u", "Integer", "count")], "outputs": [port("y", unit="count")], "notes": "Convert ignored integer requests for subtraction."},
        binary("excess_request_count", "Subtract", "Real", "Real", "Subtract ignored requests from the measured zone request count."),
        constant("zero_requests", 0.0, "count", "Only requests above the ignored count trigger a response."),
        binary("positive_request_count", "Max", "Real", "Real", "Clamp excess requests to zero."),
        {"name": "response_per_request", "block_type": "MultiplyByParameter", "category": "elementary", "inputs": [port("u", unit="count")], "outputs": [port("y", unit="normalized pressure")], "parameters": [{"name": "k", "value": 0.05, "type": "Real"}], "notes": "Raise normalized pressure by 0.05 per request above the ignored count each sample."},
        constant("maximum_response", 0.1, "normalized pressure", "Cap each sample's total pressure response."),
        binary("bounded_response", "Min", "Real", "Real", "Limit the total response in one sample."),
        constant("trim_amount", -0.03, "normalized pressure", "Trim pressure down when requests do not justify a response."),
        binary("net_pressure_reset", "Add", "Real", "Real", "Combine the trim with the bounded response."),
        {"name": "request_delay", "block_type": "Buildings.Controls.OBC.CDL.Logical.TrueDelay", "category": "elementary", "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")], "parameters": [{"name": "delayTime", "value": 720.0, "unit": "s"}, {"name": "delayOnInit", "value": True, "type": "Boolean"}], "notes": "Hold the initial pressure target for the OBC 600-second startup delay plus one 120-second reset sample after the fan is requested."},
        constant("initial_pressure_target", 0.35, "normalized pressure", "Initial pressure setpoint when fan starts or is off."),
        constant("low_pressure_target", 0.25, "normalized pressure", "Minimum normalized duct pressure target."),
        constant("high_pressure_target", 0.8, "normalized pressure", "Maximum normalized duct pressure target."),
        {"name": "pressure_previous", "block_type": "Buildings.Controls.OBC.CDL.Discrete.UnitDelay", "category": "elementary", "inputs": [port("u", unit="normalized pressure")], "outputs": [port("y", unit="normalized pressure")], "parameters": [{"name": "samplePeriod", "value": 120.0, "unit": "s"}, {"name": "y_start", "value": 0.35}], "notes": "Hold the previous sampled pressure target; this state breaks the trim-and-respond feedback loop."},
        binary("pressure_target_sum", "Add", "Real", "Real", "Apply this sample's net reset to the previous pressure target."),
        binary("pressure_above_minimum", "Max", "Real", "Real", "Limit pressure target to the editable minimum."),
        binary("pressure_below_maximum", "Min", "Real", "Real", "Limit pressure target to the editable maximum."),
        switch("pressure_candidate", "normalized pressure", "Hold the initial target until fan run time exceeds the reset delay."),
        switch("pressure_target_output", "normalized pressure", "Expose the sampled target only after the reset delay."),
        {"name": "duct_pressure_pi", "block_type": "LimPID", "category": "elementary", "inputs": [port("u_s", unit="normalized pressure"), port("u_m", unit="normalized pressure")], "outputs": [port("y", unit="fraction (0–1)")], "parameters": [{"name": "k", "value": 0.5}, {"name": "Ti", "value": 1800.0, "unit": "s"}, {"name": "yMin", "value": 0.0}, {"name": "yMax", "value": 1.0}, {"name": "reverseActing", "type": "Boolean", "value": True}], "notes": "Modulate fan speed to measured normalized duct pressure."},
        binary("fan_feedforward_max", "Max", "Real", "Real", "Use the larger of airflow feedforward and duct-pressure PI fan commands."),
        constant("fan_request_threshold", 0.05, "normalized airflow", "Keep the fan off when the AHU has no airflow request."),
        binary("fan_requested", "Greater", note="Enable the fan when requested airflow exceeds the off threshold."),
        constant("fan_off", 0.0, "fraction (0–1)", "Zero fan speed when no VAV requests air."),
        switch("fan_enable", "fraction (0–1)", "Gate the fan command with actual VAV airflow demand."),
        {"name": "minimum_oa_flow_gain", "block_type": "MultiplyByParameter", "category": "elementary", "inputs": [port("u", unit="normalized airflow")], "outputs": [port("y", unit="normalized airflow")], "parameters": [{"name": "k", "type": "Real", "value": 0.2}], "notes": "Set a teaching minimum of 20% of AHU requested airflow as outside air; not yet ASHRAE 62.1 calculation."},
        {"name": "minimum_oa_pi", "block_type": "LimPID", "category": "elementary", "inputs": [port("u_s", unit="normalized airflow"), port("u_m", unit="normalized airflow")], "outputs": [port("y", unit="fraction (0–1)")], "parameters": [{"name": "k", "value": 0.2}, {"name": "Ti", "value": 3600.0, "unit": "s"}, {"name": "yMin", "value": 0.0}, {"name": "yMax", "value": 1.0}, {"name": "reverseActing", "type": "Boolean", "value": True}], "notes": "Increase outside-air damper command when measured flow misses its minimum setpoint."},
        binary("minimum_oa_command", "Max", "Real", "Real", "Apply the greater of economizer and minimum outside-air flow commands."),
    ])
    for index, name in enumerate(zones):
        source = f"{name}_pressure_request"
        target = "request_sum_1.u1" if index == 0 else f"request_sum_{index}.u2"
        controller["connections"].append({"from": source, "to": target})
        if index >= 2:
            controller["connections"].append({"from": f"request_sum_{index - 1}.y", "to": f"request_sum_{index}.u1"})
    controller["connections"].extend({"from": source, "to": target} for source, target in [
        ("request_sum_4.y", "pressure_request_count"),
        ("request_sum_4.y", "request_count_real.u"),
        ("ignored_request_count.y", "ignored_request_real.u"),
        ("request_count_real.y", "pressure_request_sample.u"),
        ("pressure_request_sample.y", "excess_request_count.u1"),
        ("ignored_request_real.y", "excess_request_count.u2"),
        ("excess_request_count.y", "positive_request_count.u1"),
        ("zero_requests.y", "positive_request_count.u2"),
        ("positive_request_count.y", "response_per_request.u"),
        ("response_per_request.y", "bounded_response.u1"),
        ("maximum_response.y", "bounded_response.u2"),
        ("bounded_response.y", "net_pressure_reset.u1"),
        ("trim_amount.y", "net_pressure_reset.u2"),
        ("pressure_previous.y", "pressure_target_sum.u1"),
        ("net_pressure_reset.y", "pressure_target_sum.u2"),
        ("pressure_target_sum.y", "pressure_above_minimum.u1"),
        ("low_pressure_target.y", "pressure_above_minimum.u2"),
        ("pressure_above_minimum.y", "pressure_below_maximum.u1"),
        ("high_pressure_target.y", "pressure_below_maximum.u2"),
        ("pressure_below_maximum.y", "pressure_candidate.u1"),
        ("initial_pressure_target.y", "pressure_candidate.u2"),
        ("request_delay.y", "pressure_candidate.u3"),
        ("pressure_candidate.y", "pressure_previous.u"),
        ("pressure_candidate.y", "pressure_target_output.u1"),
        ("initial_pressure_target.y", "pressure_target_output.u2"),
        ("request_delay.y", "pressure_target_output.u3"),
        ("pressure_target_output.y", "duct_pressure_setpoint"),
        ("pressure_target_output.y", "duct_pressure_pi.u_s"),
        ("duct_pressure", "duct_pressure_pi.u_m"),
        ("fan_limit.y", "fan_feedforward_max.u1"),
        ("duct_pressure_pi.y", "fan_feedforward_max.u2"),
        ("flow_request", "fan_requested.u1"),
        ("fan_request_threshold.y", "fan_requested.u2"),
        ("fan_feedforward_max.y", "fan_enable.u1"),
        ("fan_off.y", "fan_enable.u2"),
        ("fan_requested.y", "fan_enable.u3"),
        ("fan_requested.y", "request_delay.u"),
        ("flow_request", "minimum_oa_flow_gain.u"),
        ("minimum_oa_flow_gain.y", "minimum_outdoor_airflow"),
        ("minimum_oa_flow_gain.y", "minimum_oa_pi.u_s"),
        ("outdoor_airflow", "minimum_oa_pi.u_m"),
        ("outdoor_air_select.y", "minimum_oa_command.u1"),
        ("minimum_oa_pi.y", "minimum_oa_command.u2"),
    ])
    for edge in controller["connections"]:
        if edge["from"] == "fan_limit.y" and edge["to"] == "fan_speed":
            edge["from"] = "fan_enable.y"
    fan_gain = next(block for block in controller["blocks"] if block["name"] == "fan_gain")
    fan_gain["parameters"][0]["value"] = 0.22
    fan_gain["notes"] = "Airflow feedforward allows startup while the duct-pressure PI loop takes control."
    controller["blocks"].extend([
        {"name": "economizer_mixed_air_pi", "block_type": "LimPID", "category": "elementary", "inputs": [port("u_s", unit="°C"), port("u_m", unit="°C")], "outputs": [port("y", unit="fraction (0–1)")], "parameters": [{"name": "k", "type": "Real", "value": 0.1}, {"name": "Ti", "type": "Real", "value": 600.0, "unit": "s"}, {"name": "yMin", "type": "Real", "value": 0.0}, {"name": "yMax", "type": "Real", "value": 1.0}, {"name": "reverseActing", "type": "Boolean", "value": False}], "notes": "Modulate outside-air fraction from measured mixed-air temperature and the supply-air target while economizing."},
        constant("economizer_low_limit", 12.0, "°C", "Conservative low-temperature guard for this simplified mixed-air model; not a G36 freeze-protection sequence."),
        binary("outdoor_below_return", "Less", note="Outdoor air can cool the return air."),
        binary("return_above_supply_target", "Greater", note="The AHU calls for cooling relative to return air."),
        binary("outdoor_above_low_limit", "Greater", note="Avoid low-temperature economizer operation."),
        binary("free_cooling_useful", "And", "Boolean", note="Temperature conditions allow free cooling."),
        binary("economizer_temperature_ok", "And", "Boolean", note="Outdoor air stays above the low limit while the fan is requested."),
        binary("economizer_enabled", "And", "Boolean", note="Enable mixed-air temperature modulation when all conditions hold."),
        switch("outdoor_air_select", "fraction (0–1)", "Select the modulating economizer fraction while enabled."),
    ])
    controller["connections"].extend({"from": source, "to": target} for source, target in [
        ("outdoor_c", "outdoor_below_return.u1"),
        ("return_air_temp", "outdoor_below_return.u2"),
        ("return_air_temp", "return_above_supply_target.u1"),
        ("supply_reset.y", "return_above_supply_target.u2"),
        ("outdoor_c", "outdoor_above_low_limit.u1"),
        ("economizer_low_limit.y", "outdoor_above_low_limit.u2"),
        ("supply_reset.y", "economizer_mixed_air_pi.u_s"),
        ("mixed_air_temp", "economizer_mixed_air_pi.u_m"),
        ("outdoor_below_return.y", "free_cooling_useful.u1"),
        ("return_above_supply_target.y", "free_cooling_useful.u2"),
        ("outdoor_above_low_limit.y", "economizer_temperature_ok.u1"),
        ("fan_requested.y", "economizer_temperature_ok.u2"),
        ("free_cooling_useful.y", "economizer_enabled.u1"),
        ("economizer_temperature_ok.y", "economizer_enabled.u2"),
        ("economizer_mixed_air_pi.y", "outdoor_air_select.u1"),
        ("minimum_outdoor_air.y", "outdoor_air_select.u2"),
        ("economizer_enabled.y", "outdoor_air_select.u3"),
        ("outdoor_air_select.y", "outdoor_air_fraction"),
    ])
    controller["connections"] = [
        edge for edge in controller["connections"]
        if not (edge["from"] == "minimum_outdoor_air.y" and edge["to"] == "outdoor_air_fraction")
    ]
    for edge in controller["connections"]:
        if edge["from"] == "outdoor_air_select.y" and edge["to"] == "outdoor_air_fraction":
            edge["from"] = "minimum_oa_command.y"
    controller["notes"] += " An editable mixed-air PI loop modulates outdoor air when temperature conditions permit free cooling; minimum outside-air flow has priority."

def add_chilled_water_reset():
    """Connect AHU coil requests to a sampled, editable plant reset sequence."""
    plant = next(block for block in graph["blocks"] if block["name"] == "plant")
    blocks, edges = plant["blocks"], plant["connections"]
    def wire(source, target):
        edges.append({"from": source, "to": target})
    def int_constant(name, value, note):
        return {"name": name, "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
                "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "count")],
                "parameters": [{"name": "k", "type": "Integer", "value": value}], "notes": note}
    def int_switch(name, note):
        return {"name": name, "block_type": "Switch", "category": "elementary",
                "inputs": [port("u1", "Integer", "count"), port("u2", "Integer", "count"), port("u3", "Boolean")],
                "outputs": [port("y", "Integer", "count")], "notes": note}
    for ahu in (1, 2):
        prefix = f"ahu_{ahu}"
        for point, source in (("cooling_valve", f"{prefix}.cooling_coil_valve_position"),
                              ("supply_setpoint_c", f"{prefix}.supply_air_temp"),
                              ("supply_actual_c", f"{prefix}_actual_supply_air_temp")):
            name = f"{prefix}_{point}"
            unit = "fraction (0–1)" if point == "cooling_valve" else "°C"
            brick = {"cooling_valve": "Cooling_Valve_Command", "supply_setpoint_c": "Supply_Air_Temperature_Setpoint",
                     "supply_actual_c": "Supply_Air_Temperature_Sensor"}[point]
            plant["inputs"].append(port(name, unit=unit, label=f"AHU {ahu} {point.replace('_', ' ').title()}", brick=brick))
            graph["connections"].append({"from": source, "to": f"plant.{name}"})
        blocks.extend([
            {"name": f"{prefix}_valve_request", "block_type": "Hysteresis", "category": "elementary",
             "inputs": [port("u", unit="fraction (0–1)")],
             "outputs": [port("y", "Boolean")], "parameters": [{"name": "uLow", "value": 0.85}, {"name": "uHigh", "value": 0.95}],
             "notes": "One chilled-water reset request when the cooling valve exceeds 95%; clear below 85%."},
            {"name": f"{prefix}_valve_request_count", "block_type": "BooleanToInteger", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Integer", "count")],
             "notes": "Convert the qualified valve request to integer zero or one."},
            binary(f"{prefix}_supply_error", "Subtract", "Real", "Real", "Actual supply air temperature minus its target."),
            constant(f"{prefix}_two_request_threshold", 2.0, "K", "Two requests after a persistent 2 K cooling deficit."),
            constant(f"{prefix}_three_request_threshold", 3.0, "K", "Three requests after a persistent 3 K cooling deficit."),
            binary(f"{prefix}_two_request_error", "Greater", note="Supply air misses its target by more than 2 K."),
            binary(f"{prefix}_three_request_error", "Greater", note="Supply air misses its target by more than 3 K."),
            *({"name": f"{prefix}_{level}_request_delay", "block_type": "TrueDelay", "category": "elementary",
               "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
               "parameters": [{"name": "delayTime", "value": 120.0, "unit": "s"}, {"name": "delayOnInit", "type": "Boolean", "value": True}],
               "notes": f"Require two minutes of {level}-request supply-air error."} for level in ("two", "three")),
            int_constant(f"{prefix}_request_two", 2, "Two chilled-water reset requests."),
            int_constant(f"{prefix}_request_three", 3, "Three chilled-water reset requests."),
            int_switch(f"{prefix}_select_two_requests", "Raise one valve request to two after a persistent supply-air error."),
            int_switch(f"{prefix}_select_three_requests", "Raise the AHU request to three after a larger persistent error."),
        ])
        for source, target in [
            (f"{prefix}_cooling_valve", f"{prefix}_valve_request.u"),
            (f"{prefix}_valve_request.y", f"{prefix}_valve_request_count.u"),
            (f"{prefix}_supply_actual_c", f"{prefix}_supply_error.u1"),
            (f"{prefix}_supply_setpoint_c", f"{prefix}_supply_error.u2"),
            (f"{prefix}_supply_error.y", f"{prefix}_two_request_error.u1"),
            (f"{prefix}_supply_error.y", f"{prefix}_three_request_error.u1"),
            (f"{prefix}_two_request_threshold.y", f"{prefix}_two_request_error.u2"),
            (f"{prefix}_three_request_threshold.y", f"{prefix}_three_request_error.u2"),
            (f"{prefix}_two_request_error.y", f"{prefix}_two_request_delay.u"),
            (f"{prefix}_three_request_error.y", f"{prefix}_three_request_delay.u"),
            (f"{prefix}_request_two.y", f"{prefix}_select_two_requests.u1"),
            (f"{prefix}_valve_request_count.y", f"{prefix}_select_two_requests.u2"),
            (f"{prefix}_two_request_delay.y", f"{prefix}_select_two_requests.u3"),
            (f"{prefix}_request_three.y", f"{prefix}_select_three_requests.u1"),
            (f"{prefix}_select_two_requests.y", f"{prefix}_select_three_requests.u2"),
            (f"{prefix}_three_request_delay.y", f"{prefix}_select_three_requests.u3"),
        ]:
            wire(source, target)
    blocks.extend([
        {"name": "chilled_reset_request_count", "block_type": "Buildings.Controls.OBC.CDL.Integers.Add", "category": "elementary",
         "inputs": [port("u1", "Integer", "count"), port("u2", "Integer", "count")],
         "outputs": [port("y", "Integer", "count")], "notes": "Sum chilled-water temperature reset requests from both AHUs."},
        {"name": "chilled_reset_request_real", "block_type": "IntegerToReal", "category": "elementary",
         "inputs": [port("u", "Integer", "count")], "outputs": [port("y", unit="count")], "notes": "Convert integer requests for sampled arithmetic."},
        constant("chilled_reset_ignored_requests", 2.0, "count", "Ignore the first two AHU requests."),
        binary("chilled_reset_excess_requests", "Subtract", "Real", "Real", "Requests above the ignored count."),
        constant("chilled_reset_zero_requests", 0.0, "count", "Clamp negative excess request count to zero."),
        binary("chilled_reset_positive_requests", "Max", "Real", "Real", "Keep only requests above the ignored count."),
        {"name": "chilled_reset_response_gain", "block_type": "MultiplyByParameter", "category": "elementary",
         "inputs": [port("u", unit="count")], "outputs": [port("y", unit="fraction (0–1)")],
         "parameters": [{"name": "k", "type": "Real", "value": 0.03}],
         "notes": "Raise normalized plant reset by 0.03 per excess request each sample."},
        constant("chilled_reset_max_response", 0.07, "fraction (0–1)", "Cap the response per sample."),
        binary("chilled_reset_bounded_response", "Min", "Real", "Real", "Cap the total response per sample."),
        constant("chilled_reset_trim", -0.02, "fraction (0–1)", "Trim reset down when requests do not justify a response."),
        binary("chilled_reset_delta", "Add", "Real", "Real", "Combine trim and response."),
        {"name": "chilled_reset_previous", "block_type": "Buildings.Controls.OBC.CDL.Discrete.UnitDelay", "category": "elementary",
         "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", unit="fraction (0–1)")],
         "parameters": [{"name": "samplePeriod", "value": 300.0, "unit": "s"}, {"name": "y_start", "value": 1.0}],
         "notes": "Hold the previous five-minute plant reset; breaks the feedback cycle."},
        binary("chilled_reset_sum", "Add", "Real", "Real", "Apply trim and response to the previous sampled reset."),
        {"name": "chilled_reset_limit", "block_type": "Limiter", "category": "elementary",
         "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", unit="fraction (0–1)")],
         "parameters": [{"name": "uMin", "value": 0.0}, {"name": "uMax", "value": 1.0}],
         "notes": "Keep the plant reset between zero and one."},
        constant("chilled_reset_initial", 1.0, "fraction (0–1)", "Restore the initial reset while chilled water is disabled."),
        switch("chilled_reset_enabled", "fraction (0–1)", "Hold the initial reset when the chilled-water loop is disabled."),
        constant("chilled_reset_temperature_start", 0.5, "fraction (0–1)",
                 "Supply-temperature reset begins after pressure reset reaches its upper limit."),
        binary("chilled_reset_temperature_excess", "Subtract", "Real", "Real",
               "Fraction of sampled plant reset above the pressure-only half."),
        constant("chilled_reset_temperature_zero", 0.0, "fraction (0–1)",
                 "Keep supply temperature at its warm limit during pressure reset."),
        binary("chilled_reset_temperature_positive", "Max", "Real", "Real",
               "Clamp temperature-reset progress to zero before the halfway point."),
        {"name": "chilled_reset_temperature_scale", "block_type": "MultiplyByParameter", "category": "elementary",
         "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", unit="fraction (0–1)")],
         "parameters": [{"name": "k", "type": "Real", "value": 2.0}],
         "notes": "Map the upper half of sampled reset to a zero-to-one temperature-reset fraction."},
        binary("chilled_reset_inverse", "Subtract", "Real", "Real", "Available warm-water reset above the minimum supply temperature."),
        {"name": "chilled_reset_warm_delta", "block_type": "MultiplyByParameter", "category": "elementary",
         "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", unit="K")],
         "parameters": [{"name": "k", "type": "Real", "value": 5.0, "unit": "K"}],
         "notes": "Five-kelvin adjustable chilled-water supply reset range for this equipment model."},
        binary("chilled_reset_supply_sum", "Add", "Real", "Real", "Add the reset allowance to the existing editable 7°C minimum target."),
    ])
    for source, target in [
        ("ahu_1_select_three_requests.y", "chilled_reset_request_count.u1"),
        ("ahu_2_select_three_requests.y", "chilled_reset_request_count.u2"),
        ("chilled_reset_request_count.y", "chilled_reset_request_real.u"),
        ("chilled_reset_request_real.y", "chilled_reset_excess_requests.u1"),
        ("chilled_reset_ignored_requests.y", "chilled_reset_excess_requests.u2"),
        ("chilled_reset_excess_requests.y", "chilled_reset_positive_requests.u1"),
        ("chilled_reset_zero_requests.y", "chilled_reset_positive_requests.u2"),
        ("chilled_reset_positive_requests.y", "chilled_reset_response_gain.u"),
        ("chilled_reset_response_gain.y", "chilled_reset_bounded_response.u1"),
        ("chilled_reset_max_response.y", "chilled_reset_bounded_response.u2"),
        ("chilled_reset_bounded_response.y", "chilled_reset_delta.u1"),
        ("chilled_reset_trim.y", "chilled_reset_delta.u2"),
        ("chilled_reset_previous.y", "chilled_reset_sum.u1"),
        ("chilled_reset_delta.y", "chilled_reset_sum.u2"),
        ("chilled_reset_sum.y", "chilled_reset_limit.u"),
        ("chilled_reset_limit.y", "chilled_reset_enabled.u1"),
        ("chilled_reset_initial.y", "chilled_reset_enabled.u2"),
        ("chilled_water_pump_needed.y", "chilled_reset_enabled.u3"),
        ("chilled_reset_enabled.y", "chilled_reset_previous.u"),
        ("chilled_reset_previous.y", "chilled_reset_temperature_excess.u1"),
        ("chilled_reset_temperature_start.y", "chilled_reset_temperature_excess.u2"),
        ("chilled_reset_temperature_excess.y", "chilled_reset_temperature_positive.u1"),
        ("chilled_reset_temperature_zero.y", "chilled_reset_temperature_positive.u2"),
        ("chilled_reset_temperature_positive.y", "chilled_reset_temperature_scale.u"),
        ("chilled_reset_initial.y", "chilled_reset_inverse.u1"),
        ("chilled_reset_temperature_scale.y", "chilled_reset_inverse.u2"),
        ("chilled_reset_inverse.y", "chilled_reset_warm_delta.u"),
        ("chilled_water_supply_setpoint.y", "chilled_reset_supply_sum.u1"),
        ("chilled_reset_warm_delta.y", "chilled_reset_supply_sum.u2"),
        ("chilled_reset_supply_sum.y", "chilled_water_supply_setpoint_c"),
        ("chilled_reset_previous.y", "chilled_water_reset_fraction"),
    ]:
        wire(source, target)
    edges[:] = [edge for edge in edges if not (edge["from"] == "chilled_water_supply_setpoint.y" and edge["to"] == "chilled_water_supply_setpoint_c")]
    edges[:] = [edge for edge in edges if not (edge["from"] == "chilled_water_supply_setpoint.y" and edge["to"] == "chilled_water_source_error.u2")]
    wire("chilled_reset_supply_sum.y", "chilled_water_source_error.u2")
    plant["outputs"].append(port("chilled_water_reset_fraction", unit="fraction (0–1)", label="Chilled Water Plant Reset", brick="Point"))
    graph["outputs"].append(port("plant_chilled_water_reset_fraction", unit="fraction (0–1)", label="Plant Chilled Water Reset", brick="Point"))
    graph["connections"].append({"from": "plant.chilled_water_reset_fraction", "to": "plant_chilled_water_reset_fraction"})
    plant["notes"] += " The G36 sample aggregates AHU chilled-water requests and resets the supply target every five minutes."


add_chilled_water_reset()


def add_hot_water_reset():
    """Add the hot-water counterpart using editable CDL request and reset blocks."""
    plant = next(block for block in graph["blocks"] if block["name"] == "plant")
    blocks, edges = plant["blocks"], plant["connections"]
    cold_blocks = copy.deepcopy(blocks)
    cold_edges = copy.deepcopy(edges)

    for ahu in (1, 2):
        prefix = f"ahu_{ahu}"
        name = f"{prefix}_heating_valve"
        plant["inputs"].append(port(name, unit="fraction (0–1)",
                                    label=f"AHU {ahu} Heating Valve Command", brick="Heating_Valve_Command"))
        graph["connections"].append({"from": f"{prefix}.heating_coil_valve_position", "to": f"plant.{name}"})
        cold_prefix = f"{prefix}_"
        request_names = {block["name"] for block in cold_blocks
                         if block["name"].startswith(cold_prefix) and
                         block["name"] not in {f"{prefix}_cooling_valve"}}
        for template in cold_blocks:
            if template["name"] not in request_names:
                continue
            cloned = copy.deepcopy(template)
            cloned["name"] = template["name"].replace(cold_prefix, f"{prefix}_hot_", 1)
            cloned["notes"] = template.get("notes", "").replace("chilled-water", "hot-water").replace("cooling", "heating")
            if cloned["name"].endswith("two_request_threshold"):
                cloned["parameters"][0]["value"] = 8.0
            elif cloned["name"].endswith("three_request_threshold"):
                cloned["parameters"][0]["value"] = 17.0
            elif cloned["name"].endswith("request_delay"):
                cloned["parameters"][0]["value"] = 300.0
            blocks.append(cloned)

        def hot_endpoint(endpoint):
            if endpoint == f"{prefix}_cooling_valve":
                return f"{prefix}_heating_valve"
            for name in request_names:
                if endpoint.startswith(name + "."):
                    return endpoint.replace(cold_prefix, f"{prefix}_hot_", 1)
            return endpoint

        for edge in cold_edges:
            target = edge["to"].split(".")[0]
            if target not in request_names:
                continue
            source, destination = hot_endpoint(edge["from"]), hot_endpoint(edge["to"])
            if destination == f"{prefix}_hot_supply_error.u1":
                source = f"{prefix}_supply_setpoint_c"
            elif destination == f"{prefix}_hot_supply_error.u2":
                source = f"{prefix}_supply_actual_c"
            edges.append({"from": source, "to": destination})

    for template in cold_blocks:
        if not template["name"].startswith("chilled_reset_"):
            continue
        if template["name"].startswith("chilled_reset_temperature_"):
            continue
        cloned = copy.deepcopy(template)
        cloned["name"] = template["name"].replace("chilled_reset_", "hot_reset_", 1)
        cloned["notes"] = template.get("notes", "").replace("chilled-water", "hot-water").replace("warm-water", "hot-water")
        if cloned["name"] == "hot_reset_previous":
            next(param for param in cloned["parameters"] if param["name"] == "y_start")["value"] = 1.0 / 6.0
            cloned["notes"] = "Hold the previous five-minute hot-water reset, starting at the 45 °C nominal target."
        elif cloned["name"] == "hot_reset_initial":
            cloned["parameters"][0]["value"] = 1.0 / 6.0
            cloned["notes"] = "Restore the 45 °C nominal target while the hot-water loop is disabled."
        elif cloned["name"] == "hot_reset_warm_delta":
            cloned["parameters"][0]["value"] = -6.0
            cloned["notes"] = "Six-kelvin hot-water reset span from 44 to 50 °C around the editable 45 °C nominal target."
        elif cloned["name"] == "hot_reset_inverse":
            cloned["notes"] = "Difference between the nominal reset fraction and the sampled fraction."
        elif cloned["name"] == "hot_reset_supply_sum":
            cloned["notes"] = "Add the reset temperature offset to the editable 45 °C nominal target."
        elif cloned["name"] == "hot_reset_enabled":
            cloned["notes"] = "Restore the initial reset when the hot-water loop is disabled."
        blocks.append(cloned)

    def hot_endpoint(endpoint):
        endpoint = endpoint.replace("chilled_reset_", "hot_reset_")
        endpoint = endpoint.replace("chilled_water_pump_needed", "hot_water_pump_needed")
        endpoint = endpoint.replace("chilled_water_supply_setpoint", "hot_water_supply_setpoint")
        endpoint = endpoint.replace("chilled_water_reset_fraction", "hot_water_reset_fraction")
        endpoint = endpoint.replace("chilled_water_source_error.u2", "hot_water_source_error.u1")
        for ahu in (1, 2):
            endpoint = endpoint.replace(f"ahu_{ahu}_select_three_requests", f"ahu_{ahu}_hot_select_three_requests")
        return endpoint

    for edge in cold_edges:
        if edge["from"].startswith("chilled_reset_") or edge["to"].startswith("chilled_reset_"):
            if "chilled_reset_temperature_" in edge["from"] or "chilled_reset_temperature_" in edge["to"]:
                continue
            edges.append({"from": hot_endpoint(edge["from"]), "to": hot_endpoint(edge["to"])})
    edges.append({"from": "hot_reset_previous.y", "to": "hot_reset_inverse.u2"})
    edges[:] = [edge for edge in edges if not (edge["from"] == "hot_water_supply_setpoint.y" and
                                                 edge["to"] in {"hot_water_supply_setpoint_c", "hot_water_source_error.u1"})]
    plant["outputs"].append(port("hot_water_reset_fraction", unit="fraction (0–1)",
                                 label="Hot Water Plant Reset", brick="Point"))
    graph["outputs"].append(port("plant_hot_water_reset_fraction", unit="fraction (0–1)",
                                 label="Plant Hot Water Reset", brick="Point"))
    graph["connections"].append({"from": "plant.hot_water_reset_fraction", "to": "plant_hot_water_reset_fraction"})
    plant["notes"] += " AHU hot-water requests reset the supply target every five minutes between 44 and 50 °C."


add_hot_water_reset()


def add_terminal_hot_water_requests():
    """Aggregate persistent VAV reheat shortfalls into the plant hot-water reset."""
    plant = next(block for block in graph["blocks"] if block["name"] == "plant")
    blocks, edges = plant["blocks"], plant["connections"]
    def wire(source, target):
        edges.append({"from": source, "to": target})
    blocks.append(constant("terminal_heat_demand_threshold", 0.5, "fraction (0–1)",
                           "Request hotter water when zone heating error exceeds one kelvin."))
    requests = []
    for index in range(1, 11):
        name = f"zone_{index:02}"
        for point, brick in (("reheat_valve", "Valve_Position_Command"), ("heating_demand", "Point")):
            input_name = f"{name}_{point}"
            plant["inputs"].append(port(input_name, unit="fraction (0–1)",
                                        label=f"Zone {index} {point.replace('_', ' ').title()}", brick=brick))
            output_name = "reheat_valve_position" if point == "reheat_valve" else point
            graph["connections"].append({"from": f"{name}.{output_name}", "to": f"plant.{input_name}"})
        blocks.extend([
            {"name": f"{name}_hot_valve_request", "block_type": "Hysteresis", "category": "elementary",
             "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "uLow", "value": 0.35}, {"name": "uHigh", "value": 0.5}],
             "notes": "VAV reheat valve must exceed 50%; request clears below 35%."},
            binary(f"{name}_hot_demand_request", "Greater", note="Zone heating demand exceeds the editable one-kelvin-equivalent threshold."),
            binary(f"{name}_hot_request_condition", "And", "Boolean", note="Both reheat valve and zone heating deficit call for hotter water."),
            {"name": f"{name}_hot_request_delay", "block_type": "TrueDelay", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "delayTime", "value": 300.0, "unit": "s"},
                            {"name": "delayOnInit", "type": "Boolean", "value": True}],
             "notes": "Require five continuous minutes of terminal reheat shortfall."},
            {"name": f"{name}_hot_request_count", "block_type": "BooleanToInteger", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Integer", "count")],
             "notes": "One hot-water plant request from this VAV."},
        ])
        for source, target in [
            (f"{name}_reheat_valve", f"{name}_hot_valve_request.u"),
            (f"{name}_heating_demand", f"{name}_hot_demand_request.u1"),
            ("terminal_heat_demand_threshold.y", f"{name}_hot_demand_request.u2"),
            (f"{name}_hot_valve_request.y", f"{name}_hot_request_condition.u1"),
            (f"{name}_hot_demand_request.y", f"{name}_hot_request_condition.u2"),
            (f"{name}_hot_request_condition.y", f"{name}_hot_request_delay.u"),
            (f"{name}_hot_request_delay.y", f"{name}_hot_request_count.u"),
        ]:
            wire(source, target)
        requests.append(f"{name}_hot_request_count.y")
    level = 0
    while len(requests) > 1:
        following = []
        for pair_index in range(0, len(requests), 2):
            if pair_index + 1 == len(requests):
                following.append(requests[pair_index])
                continue
            name = f"terminal_hot_sum_{level}_{pair_index // 2}"
            blocks.append({"name": name, "block_type": "Buildings.Controls.OBC.CDL.Integers.Add",
                           "category": "elementary", "inputs": [port("u1", "Integer", "count"), port("u2", "Integer", "count")],
                           "outputs": [port("y", "Integer", "count")],
                           "notes": "Sum VAV hot-water reset requests without a simulator-side decision."})
            wire(requests[pair_index], f"{name}.u1")
            wire(requests[pair_index + 1], f"{name}.u2")
            following.append(f"{name}.y")
        requests = following
        level += 1
    blocks.append({"name": "hot_reset_all_request_count", "block_type": "Buildings.Controls.OBC.CDL.Integers.Add",
                   "category": "elementary", "inputs": [port("u1", "Integer", "count"), port("u2", "Integer", "count")],
                   "outputs": [port("y", "Integer", "count")],
                   "notes": "Sum AHU hot-water requests and persistent terminal reheat requests."})
    wire("hot_reset_request_count.y", "hot_reset_all_request_count.u1")
    wire(requests[0], "hot_reset_all_request_count.u2")
    wire(requests[0], "terminal_hot_water_request_count")
    wire("hot_reset_all_request_count.y", "hot_reset_request_real.u")
    edges[:] = [edge for edge in edges if not (edge["from"] == "hot_reset_request_count.y" and
                                                 edge["to"] == "hot_reset_request_real.u")]
    plant["outputs"].append(port("terminal_hot_water_request_count", "Integer", "count",
                                 "Terminal Hot Water Reset Requests", "Point"))
    graph["outputs"].append(port("plant_terminal_hot_water_request_count", "Integer", "count",
                                 "Plant Terminal Hot Water Reset Requests", "Point"))
    graph["connections"].append({"from": "plant.terminal_hot_water_request_count",
                                  "to": "plant_terminal_hot_water_request_count"})
    plant["notes"] += " Persistent VAV reheat demand contributes requests to the hot-water reset."


add_terminal_hot_water_requests()


def add_supply_air_temperature_reset():
    """Use qualified VAV cooling requests to reset each AHU supply target."""
    for ahu in (1, 2):
        name = f"ahu_{ahu}"
        controller = next(block for block in graph["blocks"] if block["name"] == name)
        blocks, edges = controller["blocks"], controller["connections"]
        def wire(source, target):
            edges.append({"from": source, "to": target})
        requests = []
        for zone_index in range((ahu - 1) * 5 + 1, ahu * 5 + 1):
            zone = f"zone_{zone_index:02}"
            for signal in ("cooling_demand", "damper_position"):
                controller["inputs"].append(port(f"{zone}_{signal}", unit="fraction (0–1)",
                                                 label=f"Zone {zone_index} {signal.replace('_', ' ').title()}",
                                                 brick="Damper_Position_Command" if signal == "damper_position" else "Point"))
                graph["connections"].append({"from": f"{zone}.{signal}", "to": f"{name}.{zone}_{signal}"})
            blocks.extend([
                constant(f"{zone}_sat_demand_threshold", 0.5, "fraction (0–1)",
                         "One kelvin of cooling error qualifies this VAV's supply-temperature request."),
                binary(f"{zone}_sat_demand_high", "Greater", note="Zone cooling demand exceeds the editable threshold."),
                {"name": f"{zone}_sat_damper_open", "block_type": "Hysteresis", "category": "elementary",
                 "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", "Boolean")],
                 "parameters": [{"name": "uLow", "value": 0.85}, {"name": "uHigh", "value": 0.95}],
                 "notes": "Request a colder supply target when the VAV damper exceeds 95%; clear below 85%."},
                binary(f"{zone}_sat_request_condition", "And", "Boolean", note="Both cooling error and open damper request a colder AHU target."),
                {"name": f"{zone}_sat_request_delay", "block_type": "TrueDelay", "category": "elementary",
                 "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
                 "parameters": [{"name": "delayTime", "value": 120.0, "unit": "s"},
                                {"name": "delayOnInit", "type": "Boolean", "value": True}],
                 "notes": "Require two continuous minutes of zone cooling request."},
                {"name": f"{zone}_sat_request_count", "block_type": "BooleanToInteger", "category": "elementary",
                 "inputs": [port("u", "Boolean")], "outputs": [port("y", "Integer", "count")],
                 "notes": "One cooling supply-air temperature reset request from this VAV."},
            ])
            for source, target in [
                (f"{zone}_cooling_demand", f"{zone}_sat_demand_high.u1"),
                (f"{zone}_sat_demand_threshold.y", f"{zone}_sat_demand_high.u2"),
                (f"{zone}_damper_position", f"{zone}_sat_damper_open.u"),
                (f"{zone}_sat_demand_high.y", f"{zone}_sat_request_condition.u1"),
                (f"{zone}_sat_damper_open.y", f"{zone}_sat_request_condition.u2"),
                (f"{zone}_sat_request_condition.y", f"{zone}_sat_request_delay.u"),
                (f"{zone}_sat_request_delay.y", f"{zone}_sat_request_count.u"),
            ]:
                wire(source, target)
            requests.append(f"{zone}_sat_request_count.y")
        for index in range(4):
            name_sum = f"sat_request_sum_{index + 1}"
            blocks.append({"name": name_sum, "block_type": "Buildings.Controls.OBC.CDL.Integers.Add",
                           "category": "elementary", "inputs": [port("u1", "Integer", "count"), port("u2", "Integer", "count")],
                           "outputs": [port("y", "Integer", "count")], "notes": "Sum VAV cooling reset requests."})
            wire(requests[0] if index == 0 else f"sat_request_sum_{index}.y", f"{name_sum}.u1")
            wire(requests[index + 1], f"{name_sum}.u2")
        blocks.extend([
            {"name": "sat_request_real", "block_type": "IntegerToReal", "category": "elementary",
             "inputs": [port("u", "Integer", "count")], "outputs": [port("y", unit="count")],
             "notes": "Convert the VAV request count for sampled reset arithmetic."},
            constant("sat_ignored_requests", 2.0, "count", "Ignore the first two zone requests."),
            binary("sat_excess_requests", "Subtract", "Real", "Real", "Requests above the ignored count."),
            constant("sat_zero_requests", 0.0, "count", "Lower bound for excess requests."),
            binary("sat_positive_requests", "Max", "Real", "Real", "Only respond to requests above the ignored count."),
            {"name": "sat_response_gain", "block_type": "MultiplyByParameter", "category": "elementary",
             "inputs": [port("u", unit="count")], "outputs": [port("y", unit="K")],
             "parameters": [{"name": "k", "type": "Real", "value": -0.2, "unit": "K/request"}],
             "notes": "Lower the supply target by 0.2 K per excess request each sample."},
            constant("sat_max_response", -0.6, "K", "Cap each sample's cooling response at 0.6 K."),
            binary("sat_bounded_response", "Max", "Real", "Real", "Limit the negative temperature response."),
            constant("sat_trim", 0.1, "K", "Warm the target by 0.1 K each sample without enough requests."),
            binary("sat_delta", "Add", "Real", "Real", "Combine the warm trim and cool response."),
            {"name": "sat_previous", "block_type": "Buildings.Controls.OBC.CDL.Discrete.UnitDelay", "category": "elementary",
             "inputs": [port("u", unit="°C")], "outputs": [port("y", unit="°C")],
             "parameters": [{"name": "samplePeriod", "value": 120.0, "unit": "s"}, {"name": "y_start", "value": 16.0}],
             "notes": "Hold the previous two-minute supply-air temperature target."},
            binary("sat_target_sum", "Add", "Real", "Real", "Apply the sampled reset to the previous supply target."),
            constant("sat_minimum", 14.0, "°C", "Minimum supply-air target for this simplified coil model."),
            binary("sat_above_minimum", "Max", "Real", "Real", "Do not cool the supply target below 14 °C."),
            binary("sat_below_weather_max", "Min", "Real", "Real", "Use the existing outdoor-temperature target as the adjustable upper bound."),
            {"name": "sat_fan_delay", "block_type": "TrueDelay", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "delayTime", "value": 600.0, "unit": "s"},
                            {"name": "delayOnInit", "type": "Boolean", "value": True}],
             "notes": "Hold the outdoor-temperature target for ten minutes after fan demand starts."},
            switch("sat_candidate", "°C", "Select the reset target after fan startup."),
            switch("sat_output", "°C", "Use the sampled target while the fan is active; otherwise use the weather target."),
        ])
        for source, target in [
            ("sat_request_sum_4.y", "sat_request_count"),
            ("sat_request_sum_4.y", "sat_request_real.u"),
            ("sat_request_real.y", "sat_excess_requests.u1"),
            ("sat_ignored_requests.y", "sat_excess_requests.u2"),
            ("sat_excess_requests.y", "sat_positive_requests.u1"),
            ("sat_zero_requests.y", "sat_positive_requests.u2"),
            ("sat_positive_requests.y", "sat_response_gain.u"),
            ("sat_response_gain.y", "sat_bounded_response.u1"),
            ("sat_max_response.y", "sat_bounded_response.u2"),
            ("sat_bounded_response.y", "sat_delta.u1"),
            ("sat_trim.y", "sat_delta.u2"),
            ("sat_previous.y", "sat_target_sum.u1"),
            ("sat_delta.y", "sat_target_sum.u2"),
            ("sat_target_sum.y", "sat_above_minimum.u1"),
            ("sat_minimum.y", "sat_above_minimum.u2"),
            ("sat_above_minimum.y", "sat_below_weather_max.u1"),
            ("supply_reset.y", "sat_below_weather_max.u2"),
            ("sat_below_weather_max.y", "sat_candidate.u1"),
            ("supply_reset.y", "sat_candidate.u2"),
            ("sat_fan_delay.y", "sat_candidate.u3"),
            ("sat_candidate.y", "sat_previous.u"),
            ("sat_previous.y", "sat_output.u1"),
            ("supply_reset.y", "sat_output.u2"),
            ("sat_fan_delay.y", "sat_output.u3"),
            ("fan_requested.y", "sat_fan_delay.u"),
        ]:
            wire(source, target)
        for edge in edges:
            if edge["from"] == "supply_reset.y" and edge["to"] not in {
                "sat_below_weather_max.u2", "sat_candidate.u2", "sat_output.u2"
            }:
                edge["from"] = "sat_output.y"
        controller["outputs"].append(port("sat_request_count", "Integer", "count",
                                           "Cooling Supply Air Temperature Requests", "Point"))
        graph["outputs"].append(port(f"ahu_{ahu}_sat_request_count", "Integer", "count",
                                      f"AHU {ahu} Cooling Supply Air Temperature Requests", "Point"))
        graph["connections"].append({"from": f"ahu_{ahu}.sat_request_count",
                                      "to": f"ahu_{ahu}_sat_request_count"})
        controller["notes"] += " VAV cooling requests reset the sampled supply-air target within its outdoor-temperature bound."


add_supply_air_temperature_reset()


def add_terminal_discharge_control():
    """Reset heating discharge temperature and close the valve loop in CDL."""
    for index in range(1, 11):
        name = f"zone_{index:02}"
        zone = next(block for block in graph["blocks"] if block["name"] == name)
        blocks, edges = zone["blocks"], zone["connections"]
        blocks.extend([
            constant("discharge_max_rise", 11.0, "K", "Maximum heating discharge target above the active zone heating setpoint."),
            binary("discharge_upper", "Add", "Real", "Real", "Active heating setpoint plus the editable maximum rise."),
            binary("discharge_span", "Subtract", "Real", "Real", "Difference from AHU supply target to maximum heating discharge target."),
            constant("discharge_zero_rise", 0.0, "K", "Do not request a negative reheat span."),
            binary("discharge_positive_span", "Max", "Real", "Real", "Nonnegative discharge temperature rise available for heating."),
            {"name": "discharge_demand_double", "block_type": "MultiplyByParameter", "category": "elementary",
             "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", unit="fraction (0–1)")],
             "parameters": [{"name": "k", "type": "Real", "value": 2.0}],
             "notes": "Reach maximum discharge target at 50% heating-loop demand."},
            {"name": "discharge_demand_limit", "block_type": "Limiter", "category": "elementary",
             "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", unit="fraction (0–1)")],
             "parameters": [{"name": "uMin", "value": 0.0}, {"name": "uMax", "value": 1.0}],
             "notes": "Keep discharge reset fraction between zero and one."},
            binary("discharge_reset_rise", "Multiply", "Real", "Real", "Scale available temperature rise by heating-loop demand."),
            binary("discharge_reset_target", "Add", "Real", "Real", "Reset target from AHU supply temperature to active heating setpoint plus 11 K."),
            binary("discharge_target_cap", "Min", "Real", "Real", "Apply the editable absolute discharge temperature upper bound."),
            constant("discharge_minimum", 10.0, "°C", "Minimum discharge temperature for the low-temperature reheat exception."),
            switch("discharge_target", "°C", "Use heating reset target in heating mode; otherwise protect the 10 °C minimum."),
            {"name": "discharge_valve_pi", "block_type": "LimPID", "category": "elementary",
             "inputs": [port("u_s", unit="°C"), port("u_m", unit="°C")],
             "outputs": [port("y", unit="fraction (0–1)")],
             "parameters": [{"name": "k", "value": 0.05, "unit": "K⁻¹"}, {"name": "Ti", "value": 900.0, "unit": "s"},
                            {"name": "yMin", "value": 0.0}, {"name": "yMax", "value": 1.0},
                            {"name": "reverseActing", "type": "Boolean", "value": True}],
             "notes": "Modulate the reheat valve from measured discharge temperature; gain is tuned to the simplified 8 kW coil."},
            binary("discharge_below_minimum", "Less", note="Measured discharge is below the 10 °C protection minimum."),
            binary("discharge_valve_enable", "Or", "Boolean", "Boolean", "Enable reheat during a heating call or a low discharge temperature."),
            constant("discharge_valve_closed", 0.0, "fraction (0–1)", "Close reheat valve outside heating and low-temperature protection."),
            switch("discharge_valve_select", "fraction (0–1)", "Gate the discharge-temperature PI valve command."),
        ])
        unit_map = {
            "discharge_upper": (("°C", "K"), "°C"),
            "discharge_span": (("°C", "°C"), "K"),
            "discharge_positive_span": (("K", "K"), "K"),
            "discharge_reset_rise": (("K", "fraction (0–1)"), "K"),
            "discharge_reset_target": (("°C", "K"), "°C"),
            "discharge_target_cap": (("°C", "°C"), "°C"),
            "discharge_below_minimum": (("°C", "°C"), "Boolean"),
            "discharge_valve_enable": (("Boolean", "Boolean"), "Boolean"),
        }
        for block in blocks:
            if block["name"] in unit_map:
                input_units, output_unit = unit_map[block["name"]]
                for connector, unit in zip(block["inputs"], input_units):
                    connector["unit"] = unit
                block["outputs"][0]["unit"] = output_unit
        edges.extend({"from": source, "to": target} for source, target in [
            ("heat_setpoint_active.y", "discharge_upper.u1"),
            ("discharge_max_rise.y", "discharge_upper.u2"),
            ("discharge_upper.y", "discharge_span.u1"),
            ("supply_air_temp", "discharge_span.u2"),
            ("discharge_span.y", "discharge_positive_span.u1"),
            ("discharge_zero_rise.y", "discharge_positive_span.u2"),
            ("heat_pi.y", "discharge_demand_double.u"),
            ("discharge_demand_double.y", "discharge_demand_limit.u"),
            ("discharge_positive_span.y", "discharge_reset_rise.u1"),
            ("discharge_demand_limit.y", "discharge_reset_rise.u2"),
            ("supply_air_temp", "discharge_reset_target.u1"),
            ("discharge_reset_rise.y", "discharge_reset_target.u2"),
            ("discharge_reset_target.y", "discharge_target_cap.u1"),
            ("maximum_discharge_temp.y", "discharge_target_cap.u2"),
            ("discharge_target_cap.y", "discharge_target.u1"),
            ("discharge_minimum.y", "discharge_target.u2"),
            ("heating_mode.y", "discharge_target.u3"),
            ("discharge_target.y", "discharge_valve_pi.u_s"),
            ("discharge_temp", "discharge_valve_pi.u_m"),
            ("discharge_temp", "discharge_below_minimum.u1"),
            ("discharge_minimum.y", "discharge_below_minimum.u2"),
            ("heating_mode.y", "discharge_valve_enable.u1"),
            ("discharge_below_minimum.y", "discharge_valve_enable.u2"),
            ("discharge_valve_pi.y", "discharge_valve_select.u1"),
            ("discharge_valve_closed.y", "discharge_valve_select.u2"),
            ("discharge_valve_enable.y", "discharge_valve_select.u3"),
            ("discharge_target.y", "discharge_air_setpoint_c"),
        ])
        for edge in edges:
            if edge["from"] == "reheat_select.y" and edge["to"] == "reheat_capped.u1":
                edge["from"] = "discharge_valve_select.y"
        zone["outputs"].append(port("discharge_air_setpoint_c", unit="°C", label="Discharge Air Temperature Setpoint",
                                    brick="Discharge_Air_Temperature_Setpoint"))
        graph["outputs"].append(port(f"{name}_discharge_air_setpoint_c", unit="°C",
                                     label=f"Zone {index} Discharge Air Temperature Setpoint",
                                     brick="Discharge_Air_Temperature_Setpoint"))
        graph["connections"].append({"from": f"{name}.discharge_air_setpoint_c",
                                      "to": f"{name}_discharge_air_setpoint_c"})
        zone["notes"] += " Heating demand resets the discharge target; a PI loop uses measured discharge temperature to command reheat."


add_terminal_discharge_control()


def add_terminal_heating_airflow_interlock():
    """Only increase heating airflow after the discharge air can warm the zone."""
    for index in range(1, 11):
        zone = next(block for block in graph["blocks"] if block["name"] == f"zone_{index:02}")
        blocks, edges = zone["blocks"], zone["connections"]
        blocks.extend([
            constant("heating_airflow_midpoint", 0.5, "fraction (0–1)", "The first half of heating demand resets discharge temperature, not airflow."),
            binary("heating_airflow_excess", "Subtract", "Real", "Real", "Heating demand above the 50% discharge-reset stage."),
            {"name": "heating_airflow_double", "block_type": "MultiplyByParameter", "category": "elementary",
             "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", unit="fraction (0–1)")],
             "parameters": [{"name": "k", "value": 2.0, "type": "Real"}],
             "notes": "Map the upper half of heating demand to the full heating airflow range."},
            {"name": "heating_airflow_limit", "block_type": "Limiter", "category": "elementary",
             "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", unit="fraction (0–1)")],
             "parameters": [{"name": "uMin", "value": 0.0}, {"name": "uMax", "value": 1.0}],
             "notes": "Keep the upper-stage heating airflow fraction in its valid range."},
            constant("heating_airflow_margin", 3.0, "K", "Only increase heating flow when discharge exceeds room temperature by 3 K."),
            binary("heating_airflow_threshold", "Add", "Real", "Real", "Room temperature plus the editable warm-discharge margin."),
            binary("heating_airflow_warm", "Greater", note="Measured discharge air is warm enough to increase primary flow."),
            binary("heating_airflow_enabled", "And", "Boolean", "Boolean", "Only raise airflow in heating mode with warm discharge air."),
            constant("heating_airflow_zero", 0.0, "fraction (0–1)", "Hold heating airflow at its active minimum until the warm-discharge condition is met."),
            switch("heating_airflow_select", "fraction (0–1)", "Apply the upper-stage heating airflow request only after discharge is warm."),
            constant("heating_warmup_full", 1.0, "fraction (0–1)", "Use the heating airflow maximum during preconditioning warm-up."),
            switch("heating_warmup_airflow", "fraction (0–1)", "In preconditioning, use the heating maximum instead of the occupied-stage airflow request."),
        ])
        unit_map = {
            "heating_airflow_excess": (("fraction (0–1)", "fraction (0–1)"), "fraction (0–1)"),
            "heating_airflow_threshold": (("°C", "K"), "°C"),
            "heating_airflow_warm": (("°C", "°C"), "Boolean"),
            "heating_airflow_enabled": (("Boolean", "Boolean"), "Boolean"),
        }
        for block in blocks:
            if block["name"] in unit_map:
                input_units, output_unit = unit_map[block["name"]]
                for connector, unit in zip(block["inputs"], input_units):
                    connector["unit"] = unit
                block["outputs"][0]["unit"] = output_unit
        edges.extend({"from": source, "to": target} for source, target in [
            ("heat_pi.y", "heating_airflow_excess.u1"),
            ("heating_airflow_midpoint.y", "heating_airflow_excess.u2"),
            ("heating_airflow_excess.y", "heating_airflow_double.u"),
            ("heating_airflow_double.y", "heating_airflow_limit.u"),
            ("room_temp", "heating_airflow_threshold.u1"),
            ("heating_airflow_margin.y", "heating_airflow_threshold.u2"),
            ("discharge_temp", "heating_airflow_warm.u1"),
            ("heating_airflow_threshold.y", "heating_airflow_warm.u2"),
            ("heating_mode.y", "heating_airflow_enabled.u1"),
            ("heating_airflow_warm.y", "heating_airflow_enabled.u2"),
            ("heating_airflow_limit.y", "heating_airflow_select.u1"),
            ("heating_airflow_zero.y", "heating_airflow_select.u2"),
            ("heating_airflow_enabled.y", "heating_airflow_select.u3"),
            ("heating_warmup_full.y", "heating_warmup_airflow.u1"),
            ("heating_airflow_select.y", "heating_warmup_airflow.u2"),
            ("preconditioning_mode", "heating_warmup_airflow.u3"),
        ])
        for edge in edges:
            if edge["from"] == "heat_pi.y" and edge["to"] == "heating_damper_gain.u":
                edge["from"] = "heating_warmup_airflow.y"
        zone["notes"] += " Heating airflow rises only after the discharge is 3 K warmer than the room."


add_terminal_heating_airflow_interlock()


def add_terminal_damper_feedback():
    """Modulate damper position from measured primary airflow and the CDL setpoint."""
    for index in range(1, 11):
        zone = next(block for block in graph["blocks"] if block["name"] == f"zone_{index:02}")
        blocks, edges = zone["blocks"], zone["connections"]
        blocks.extend([
            {"name": "damper_flow_pi", "block_type": "LimPID", "category": "elementary",
             "inputs": [port("u_s", unit="normalized airflow"), port("u_m", unit="normalized airflow")],
             "outputs": [port("y", unit="fraction (0–1)")],
             "parameters": [{"name": "k", "value": 0.5}, {"name": "Ti", "value": 300.0, "unit": "s"},
                            {"name": "yMin", "value": 0.0}, {"name": "yMax", "value": 1.0},
                            {"name": "reverseActing", "type": "Boolean", "value": True}],
             "notes": "Modulate VAV damper to track measured primary airflow against its editable setpoint."},
            constant("damper_flow_threshold", 0.01, "normalized airflow", "Close the damper when the airflow setpoint is effectively zero."),
            binary("damper_flow_requested", "Greater", note="The terminal has a nonzero primary-airflow setpoint."),
            constant("damper_closed", 0.0, "fraction (0–1)", "Command a closed damper when primary airflow is not requested."),
            switch("damper_feedback_output", "fraction (0–1)", "Use the measured-airflow PI command only while primary air is requested."),
            {"name": "measured_airflow_passthrough", "block_type": "MultiplyByParameter", "category": "elementary",
             "inputs": [port("u", unit="normalized airflow")], "outputs": [port("y", unit="normalized airflow")],
             "parameters": [{"name": "k", "value": 1.0, "type": "Real"}],
             "notes": "Expose measured primary airflow separately from its commanded setpoint."},
        ])
        block = next(item for item in blocks if item["name"] == "damper_flow_requested")
        for connector in block["inputs"]:
            connector["unit"] = "normalized airflow"
        block["outputs"][0]["unit"] = "Boolean"
        edges.extend({"from": source, "to": target} for source, target in [
            ("airflow_request_sum.y", "damper_flow_pi.u_s"),
            ("measured_airflow", "damper_flow_pi.u_m"),
            ("airflow_request_sum.y", "damper_flow_requested.u1"),
            ("damper_flow_threshold.y", "damper_flow_requested.u2"),
            ("damper_flow_pi.y", "damper_feedback_output.u1"),
            ("damper_closed.y", "damper_feedback_output.u2"),
            ("damper_flow_requested.y", "damper_feedback_output.u3"),
            ("measured_airflow", "measured_airflow_passthrough.u"),
            ("measured_airflow_passthrough.y", "airflow"),
        ])
        for edge in edges:
            if edge["from"] == "heating_damper_select.y" and edge["to"] in ("damper_position", "damper_nearly_open.u1"):
                edge["from"] = "damper_feedback_output.y"
        edges[:] = [edge for edge in edges if not (edge["from"] == "airflow_sum.y" and edge["to"] == "airflow")]
        zone["notes"] += " The damper PI tracks measured primary airflow; its setpoint remains separate from the physical delivery."


add_terminal_damper_feedback()


def add_ahu_freeze_stage_one():
    """Give both AHUs a timed, graph-owned first freeze-protection stage."""
    plant = next(block for block in graph["blocks"] if block["name"] == "plant")
    for index in (1, 2):
        name = f"ahu_{index}"
        ahu = next(block for block in graph["blocks"] if block["name"] == name)
        blocks, edges = ahu["blocks"], ahu["connections"]
        blocks.extend([
            constant("freeze_stage_one_on", 4.4, "°C", "Stage 1 begins after supply air stays below 4.4 °C for five minutes."),
            constant("freeze_stage_one_off", 7.0, "°C", "Stage 1 clears after supply air stays above 7 °C for five minutes."),
            binary("freeze_stage_one_cold", "Less", note="Detect low measured supply-air temperature."),
            binary("freeze_stage_one_warm", "Greater", note="Detect recovered measured supply-air temperature."),
            {"name": "freeze_stage_one_on_delay", "block_type": "TrueDelay", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "delayTime", "value": 300.0, "unit": "s"},
                            {"name": "delayOnInit", "type": "Boolean", "value": True}],
             "notes": "Require five continuous minutes below the Stage 1 temperature threshold."},
            {"name": "freeze_stage_one_off_delay", "block_type": "TrueDelay", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "delayTime", "value": 300.0, "unit": "s"},
                            {"name": "delayOnInit", "type": "Boolean", "value": True}],
             "notes": "Require five continuous minutes above the recovery threshold."},
            {"name": "freeze_stage_one_latch", "block_type": "Latch", "category": "elementary",
             "inputs": [port("u", "Boolean"), port("clr", "Boolean")],
             "outputs": [port("y", "Boolean")],
             "notes": "Hold Stage 1 during the supply-temperature recovery interval."},
            switch("freeze_stage_one_outdoor_air", "fraction (0–1)",
                   "Override the outdoor-air command to its editable minimum while Stage 1 is active."),
            binary("freeze_stage_one_heating_enable", "Or", "Boolean", "Boolean",
                   "Keep the heating coil enabled during Stage 1 even if normal mixed-air logic would disable it."),
            {"name": "freeze_stage_one_request_integer", "block_type": "BooleanToInteger", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Integer", "count")],
             "notes": "Convert Stage 1 status into a hot-water plant request."},
            {"name": "freeze_stage_one_two_requests", "block_type": "Buildings.Controls.OBC.CDL.Integers.Add",
             "category": "elementary", "inputs": [port("u1", "Integer", "count"), port("u2", "Integer", "count")],
             "outputs": [port("y", "Integer", "count")],
             "notes": "Issue two hot-water plant requests during Stage 1."},
        ])
        edges.extend({"from": source, "to": target} for source, target in [
            ("actual_supply_air_temp", "freeze_stage_one_cold.u1"),
            ("freeze_stage_one_on.y", "freeze_stage_one_cold.u2"),
            ("actual_supply_air_temp", "freeze_stage_one_warm.u1"),
            ("freeze_stage_one_off.y", "freeze_stage_one_warm.u2"),
            ("freeze_stage_one_cold.y", "freeze_stage_one_on_delay.u"),
            ("freeze_stage_one_warm.y", "freeze_stage_one_off_delay.u"),
            ("freeze_stage_one_on_delay.y", "freeze_stage_one_latch.u"),
            ("freeze_stage_one_off_delay.y", "freeze_stage_one_latch.clr"),
            ("minimum_outdoor_air.y", "freeze_stage_one_outdoor_air.u1"),
            ("minimum_oa_command.y", "freeze_stage_one_outdoor_air.u2"),
            ("freeze_stage_one_latch.y", "freeze_stage_one_outdoor_air.u3"),
            ("heating_coil_needed.y", "freeze_stage_one_heating_enable.u1"),
            ("freeze_stage_one_latch.y", "freeze_stage_one_heating_enable.u2"),
            ("freeze_stage_one_latch.y", "freeze_stage_one"),
            ("freeze_stage_one_latch.y", "freeze_stage_one_request_integer.u"),
            ("freeze_stage_one_request_integer.y", "freeze_stage_one_two_requests.u1"),
            ("freeze_stage_one_request_integer.y", "freeze_stage_one_two_requests.u2"),
            ("freeze_stage_one_two_requests.y", "freeze_hot_water_request"),
        ])
        for edge in edges:
            if edge["from"] == "minimum_oa_command.y" and edge["to"] == "outdoor_air_fraction":
                edge["from"] = "freeze_stage_one_outdoor_air.y"
            if edge["from"] == "heating_coil_needed.y" and edge["to"] == "heating_coil_enable.u3":
                edge["from"] = "freeze_stage_one_heating_enable.y"
        ahu["outputs"].extend([
            port("freeze_stage_one", "Boolean", "unitless", "Freeze Protection Stage 1", "Point"),
            port("freeze_hot_water_request", "Integer", "count", "Freeze Hot Water Plant Requests", "Point"),
        ])
        graph["outputs"].extend([
            port(f"{name}_freeze_stage_one", "Boolean", "unitless", f"AHU {index} Freeze Protection Stage 1", "Point"),
            port(f"{name}_freeze_hot_water_request", "Integer", "count", f"AHU {index} Freeze Hot Water Plant Requests", "Point"),
        ])
        graph["connections"].extend([
            {"from": f"{name}.freeze_stage_one", "to": f"{name}_freeze_stage_one"},
            {"from": f"{name}.freeze_hot_water_request", "to": f"{name}_freeze_hot_water_request"},
            {"from": f"{name}.freeze_hot_water_request", "to": f"plant.{name}_freeze_hot_water_request"},
        ])
        plant["inputs"].append(port(f"{name}_freeze_hot_water_request", "Integer", "count",
                                    f"AHU {index} Freeze Hot Water Plant Requests", "Point"))
        ahu["notes"] += " A timed Stage 1 freeze response reduces outdoor air to its minimum and sends two hot-water requests."

    plant["blocks"].extend([
        {"name": "freeze_hot_water_request_sum", "block_type": "Buildings.Controls.OBC.CDL.Integers.Add",
         "category": "elementary", "inputs": [port("u1", "Integer", "count"), port("u2", "Integer", "count")],
         "outputs": [port("y", "Integer", "count")],
         "notes": "Sum Stage 1 freeze requests from both AHUs."},
        {"name": "hot_reset_with_freeze_requests", "block_type": "Buildings.Controls.OBC.CDL.Integers.Add",
         "category": "elementary", "inputs": [port("u1", "Integer", "count"), port("u2", "Integer", "count")],
         "outputs": [port("y", "Integer", "count")],
         "notes": "Add freeze requests before the sampled hot-water reset."},
        {"name": "freeze_request_count_real", "block_type": "IntegerToReal", "category": "elementary",
         "inputs": [port("u", "Integer", "count")], "outputs": [port("y", unit="count")],
         "notes": "Convert freeze request count for the plant enable comparison."},
        constant("freeze_request_zero", 0.0, "count", "No active freeze protection requests."),
        binary("freeze_hot_water_active", "Greater", note="At least one AHU requests freeze protection."),
        binary("freeze_hot_water_pump_enable", "Or", "Boolean", "Boolean",
               "Run the hot-water pump when ordinary load or AHU freeze protection requires it."),
        constant("freeze_hot_water_nominal", 45.0, "°C", "Keep the hot-water target at least 45 °C during AHU freeze protection."),
        binary("freeze_hot_water_minimum_target", "Max", "Real", "Real",
               "Prevent hot-water reset from trimming below the freeze-protection target."),
        switch("freeze_hot_water_target", "°C", "Apply the freeze minimum only while an AHU requests protection."),
    ])
    plant["connections"] = [edge for edge in plant["connections"] if not (
        (edge["from"] == "hot_reset_all_request_count.y" and edge["to"] == "hot_reset_request_real.u") or
        (edge["from"] == "hot_water_pump_needed.y" and edge["to"] in
         ("hot_water_enable", "hot_water_pump_select.u3", "hot_reset_enabled.u3")) or
        (edge["from"] == "hot_reset_supply_sum.y" and edge["to"] == "hot_water_supply_setpoint_c"))]
    plant["connections"].extend({"from": source, "to": target} for source, target in [
        ("ahu_1_freeze_hot_water_request", "freeze_hot_water_request_sum.u1"),
        ("ahu_2_freeze_hot_water_request", "freeze_hot_water_request_sum.u2"),
        ("hot_reset_all_request_count.y", "hot_reset_with_freeze_requests.u1"),
        ("freeze_hot_water_request_sum.y", "hot_reset_with_freeze_requests.u2"),
        ("hot_reset_with_freeze_requests.y", "hot_reset_request_real.u"),
        ("freeze_hot_water_request_sum.y", "freeze_request_count_real.u"),
        ("freeze_request_count_real.y", "freeze_hot_water_active.u1"),
        ("freeze_request_zero.y", "freeze_hot_water_active.u2"),
        ("hot_water_pump_needed.y", "freeze_hot_water_pump_enable.u1"),
        ("freeze_hot_water_active.y", "freeze_hot_water_pump_enable.u2"),
        ("freeze_hot_water_pump_enable.y", "hot_water_enable"),
        ("freeze_hot_water_pump_enable.y", "hot_water_pump_select.u3"),
        ("freeze_hot_water_pump_enable.y", "hot_reset_enabled.u3"),
        ("hot_reset_supply_sum.y", "freeze_hot_water_minimum_target.u1"),
        ("freeze_hot_water_nominal.y", "freeze_hot_water_minimum_target.u2"),
        ("freeze_hot_water_minimum_target.y", "freeze_hot_water_target.u1"),
        ("hot_reset_supply_sum.y", "freeze_hot_water_target.u2"),
        ("freeze_hot_water_active.y", "freeze_hot_water_target.u3"),
        ("freeze_hot_water_target.y", "hot_water_supply_setpoint_c"),
    ])
    plant["notes"] += " AHU Stage 1 freeze protection contributes hot-water reset requests."


add_ahu_freeze_stage_one()


def add_ahu_freeze_stage_two():
    """Interrupt outdoor ventilation for one hour after a deeper timed freeze fault."""
    for index in (1, 2):
        name = f"ahu_{index}"
        ahu = next(block for block in graph["blocks"] if block["name"] == name)
        blocks, edges = ahu["blocks"], ahu["connections"]
        blocks.extend([
            constant("freeze_stage_two_on", 3.3, "°C", "Stage 2 begins after supply air stays below 3.3 °C for five minutes."),
            binary("freeze_stage_two_cold", "Less", note="Detect the deeper measured supply-air temperature fault."),
            {"name": "freeze_stage_two_on_delay", "block_type": "TrueDelay", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "delayTime", "value": 300.0, "unit": "s"},
                            {"name": "delayOnInit", "type": "Boolean", "value": True}],
             "notes": "Require five continuous minutes below the Stage 2 temperature threshold."},
            {"name": "freeze_stage_two_latch", "block_type": "Latch", "category": "elementary",
             "inputs": [port("u", "Boolean"), port("clr", "Boolean")],
             "outputs": [port("y", "Boolean")],
             "notes": "Hold the outdoor-air interruption until its one-hour timer has elapsed."},
            {"name": "freeze_stage_two_to_integer", "block_type": "BooleanToInteger", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Integer", "count")],
             "notes": "Convert Stage 2 status for a sampled timer feedback path."},
            {"name": "freeze_stage_two_to_real", "block_type": "IntegerToReal", "category": "elementary",
             "inputs": [port("u", "Integer", "count")], "outputs": [port("y", unit="count")],
             "notes": "Convert the sampled Stage 2 status to a Real signal for the duration comparison."},
            {"name": "freeze_stage_two_previous", "block_type": "UnitDelay", "category": "elementary",
             "inputs": [port("u", unit="count")], "outputs": [port("y", unit="count")],
             "parameters": [{"name": "samplePeriod", "value": 60.0, "unit": "s"},
                            {"name": "y_start", "value": 0}],
             "notes": "Break the Stage 2 latch/timer feedback loop using its prior sampled status."},
            constant("freeze_stage_two_zero", 0.0, "count", "Zero means the prior Stage 2 state was inactive."),
            binary("freeze_stage_two_previous_active", "Greater", note="Was Stage 2 active at the prior sample?"),
            {"name": "freeze_stage_two_hour", "block_type": "TrueDelay", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "delayTime", "value": 3540.0, "unit": "s"},
                            {"name": "delayOnInit", "type": "Boolean", "value": True}],
             "notes": "Clear Stage 2 after one hour including the sampled feedback and timer step."},
            constant("freeze_stage_two_closed", 0.0, "fraction (0–1)",
                     "Close all outdoor-air paths while minimum ventilation is interrupted."),
            switch("freeze_stage_two_outdoor_air", "fraction (0–1)",
                   "Close outdoor air during Stage 2; otherwise restore Stage 1 or normal control."),
            {"name": "freeze_stage_two_alarm_integer", "block_type": "BooleanToInteger", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Integer", "level")],
             "notes": "Convert the active ventilation interruption to alarm level one."},
            {"name": "freeze_stage_two_alarm_double", "block_type": "Buildings.Controls.OBC.CDL.Integers.Add",
             "category": "elementary", "inputs": [port("u1", "Integer", "level"), port("u2", "Integer", "level")],
             "outputs": [port("y", "Integer", "level")],
             "notes": "Build Level 2 from the active Stage 2 alarm bit."},
            {"name": "freeze_stage_two_alarm_three", "block_type": "Buildings.Controls.OBC.CDL.Integers.Add",
             "category": "elementary", "inputs": [port("u1", "Integer", "level"), port("u2", "Integer", "level")],
             "outputs": [port("y", "Integer", "level")],
             "notes": "Report Level 3 while minimum ventilation is interrupted."},
        ])
        edges.extend({"from": source, "to": target} for source, target in [
            ("actual_supply_air_temp", "freeze_stage_two_cold.u1"),
            ("freeze_stage_two_on.y", "freeze_stage_two_cold.u2"),
            ("freeze_stage_two_cold.y", "freeze_stage_two_on_delay.u"),
            ("freeze_stage_two_on_delay.y", "freeze_stage_two_latch.u"),
            ("freeze_stage_two_latch.y", "freeze_stage_two_to_integer.u"),
            ("freeze_stage_two_to_integer.y", "freeze_stage_two_to_real.u"),
            ("freeze_stage_two_to_real.y", "freeze_stage_two_previous.u"),
            ("freeze_stage_two_previous.y", "freeze_stage_two_previous_active.u1"),
            ("freeze_stage_two_zero.y", "freeze_stage_two_previous_active.u2"),
            ("freeze_stage_two_previous_active.y", "freeze_stage_two_hour.u"),
            ("freeze_stage_two_hour.y", "freeze_stage_two_latch.clr"),
            ("freeze_stage_two_closed.y", "freeze_stage_two_outdoor_air.u1"),
            ("freeze_stage_one_outdoor_air.y", "freeze_stage_two_outdoor_air.u2"),
            ("freeze_stage_two_latch.y", "freeze_stage_two_outdoor_air.u3"),
            ("freeze_stage_two_latch.y", "freeze_stage_two"),
            ("freeze_stage_two_latch.y", "freeze_stage_two_alarm_integer.u"),
            ("freeze_stage_two_alarm_integer.y", "freeze_stage_two_alarm_double.u1"),
            ("freeze_stage_two_alarm_integer.y", "freeze_stage_two_alarm_double.u2"),
            ("freeze_stage_two_alarm_double.y", "freeze_stage_two_alarm_three.u1"),
            ("freeze_stage_two_alarm_integer.y", "freeze_stage_two_alarm_three.u2"),
            ("freeze_stage_two_alarm_three.y", "freeze_ventilation_alarm_level"),
        ])
        for edge in edges:
            if edge["from"] == "freeze_stage_one_outdoor_air.y" and edge["to"] == "outdoor_air_fraction":
                edge["from"] = "freeze_stage_two_outdoor_air.y"
        ahu["outputs"].extend([
            port("freeze_stage_two", "Boolean", "unitless", "Freeze Protection Stage 2", "Point"),
            port("freeze_ventilation_alarm_level", "Integer", "level", "Freeze Ventilation Alarm Level", "Point"),
        ])
        graph["outputs"].extend([
            port(f"{name}_freeze_stage_two", "Boolean", "unitless", f"AHU {index} Freeze Protection Stage 2", "Point"),
            port(f"{name}_freeze_ventilation_alarm_level", "Integer", "level", f"AHU {index} Freeze Ventilation Alarm Level", "Point"),
        ])
        graph["connections"].extend([
            {"from": f"{name}.freeze_stage_two", "to": f"{name}_freeze_stage_two"},
            {"from": f"{name}.freeze_ventilation_alarm_level", "to": f"{name}_freeze_ventilation_alarm_level"},
        ])
        ahu["notes"] += " Stage 2 closes outdoor air for one hour and reports a Level 3 ventilation alarm."


add_ahu_freeze_stage_two()


def add_ahu_freeze_stage_three():
    """Latch the final AHU freeze shutdown and connect its pump commands."""
    plant = next(block for block in graph["blocks"] if block["name"] == "plant")
    for index in (1, 2):
        name = f"ahu_{index}"
        ahu = next(block for block in graph["blocks"] if block["name"] == name)
        blocks, edges = ahu["blocks"], ahu["connections"]
        for signal, label in (("freeze_stat_trip", "Freeze Stat Trip"),
                              ("freeze_reset", "Freeze Shutdown Operator Reset")):
            graph["inputs"].append(port(f"{name}_{signal}", "Boolean", "unitless",
                                        f"AHU {index} {label}", "Point",
                                        "Raw test or operator input; the CDL graph decides all shutdown actions"))
            ahu["inputs"].append(port(signal, "Boolean", "unitless", label, "Point"))
            graph["connections"].append({"from": f"{name}_{signal}", "to": f"{name}.{signal}"})
        blocks.extend([
            constant("freeze_stage_three_fifteen_threshold", 3.3, "°C",
                     "A supply temperature below 3.3 °C for 15 minutes requires shutdown."),
            constant("freeze_stage_three_five_threshold", 1.0, "°C",
                     "A supply temperature below 1 °C for five minutes requires shutdown."),
            binary("freeze_stage_three_below_fifteen", "Less", note="Detect the sustained 3.3 °C shutdown fault."),
            binary("freeze_stage_three_below_five", "Less", note="Detect the deeper 1 °C shutdown fault."),
            {"name": "freeze_stage_three_fifteen_delay", "block_type": "TrueDelay", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "delayTime", "value": 900.0, "unit": "s"},
                            {"name": "delayOnInit", "type": "Boolean", "value": True}],
             "notes": "Require 15 continuous minutes below 3.3 °C."},
            {"name": "freeze_stage_three_five_delay", "block_type": "TrueDelay", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "delayTime", "value": 300.0, "unit": "s"},
                            {"name": "delayOnInit", "type": "Boolean", "value": True}],
             "notes": "Require five continuous minutes below 1 °C."},
            binary("freeze_stage_three_sensor_fault", "Or", "Boolean", "Boolean",
                   "Either sustained low-temperature condition requires shutdown."),
            binary("freeze_stage_three_trip", "Or", "Boolean", "Boolean",
                   "A freeze-stat trip or low-temperature fault requires shutdown."),
            constant("freeze_stage_three_reset_temperature", 7.0, "°C",
                     "Permit operator reset only after supply air has recovered above 7 °C."),
            binary("freeze_stage_three_reset_warm", "Greater", note="Supply air is warm enough to reset."),
            {"name": "freeze_stage_three_stat_normal", "block_type": "Not", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "notes": "A freeze-stat trip must clear before operator reset."},
            binary("freeze_stage_three_reset_safe", "And", "Boolean", "Boolean",
                   "Both supply temperature and freeze-stat status permit reset."),
            binary("freeze_stage_three_reset_allowed", "And", "Boolean", "Boolean",
                   "Accept an operator reset only when the fault has cleared."),
            {"name": "freeze_stage_three_latch", "block_type": "Latch", "category": "elementary",
             "inputs": [port("u", "Boolean"), port("clr", "Boolean")],
             "outputs": [port("y", "Boolean")],
             "notes": "Hold freeze shutdown until a safe operator reset; the harness cannot clear it."},
            constant("freeze_stage_three_fan_off", 0.0, "fraction (0–1)", "Stop the supply fan during shutdown."),
            switch("freeze_stage_three_fan", "fraction (0–1)", "Override the graph fan command to zero."),
            constant("freeze_stage_three_outdoor_closed", 0.0, "fraction (0–1)", "Close outdoor air during shutdown."),
            switch("freeze_stage_three_outdoor_air", "fraction (0–1)", "Override Stage 2 and normal outdoor-air commands."),
            constant("freeze_stage_three_cooling_open", 1.0, "fraction (0–1)",
                     "Open the cooling-coil valve fully for coil freeze protection."),
            switch("freeze_stage_three_cooling_valve", "fraction (0–1)",
                   "Override the cooling-coil valve fully open during shutdown."),
            binary("freeze_stage_three_measured_high", "Max", "Real", "Real",
                   "Protect the warmer of measured supply and mixed air temperatures."),
            constant("freeze_stage_three_heating_target", 27.0, "°C",
                     "Target 27 °C for the higher of measured supply and mixed air during shutdown."),
            {"name": "freeze_stage_three_heating_pi", "block_type": "LimPID", "category": "elementary",
             "inputs": [port("u_s", unit="°C"), port("u_m", unit="°C")],
             "outputs": [port("y", unit="fraction (0–1)")],
             "parameters": [{"name": "k", "value": 0.1}, {"name": "Ti", "value": 300.0, "unit": "s"},
                            {"name": "yMin", "value": 0.0}, {"name": "yMax", "value": 1.0},
                            {"name": "reverseActing", "type": "Boolean", "value": True}],
             "notes": "Modulate the heating coil to protect the higher measured air temperature."},
            switch("freeze_stage_three_heating_valve", "fraction (0–1)",
                   "Use the shutdown heating PI while Stage 3 is latched."),
            binary("freeze_stage_three_hot_request", "Or", "Boolean", "Boolean",
                   "Stages 1 and 3 both issue two hot-water plant requests."),
            {"name": "freeze_stage_three_alarm_two", "block_type": "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
             "category": "elementary", "inputs": [], "outputs": [port("y", "Integer", "level")],
             "parameters": [{"name": "k", "type": "Integer", "value": 2}],
             "notes": "Level 2 alarm for a latched freeze shutdown."},
            {"name": "freeze_stage_three_alarm_select", "block_type": "Switch", "category": "elementary",
             "inputs": [port("u1", "Integer", "level"), port("u2", "Integer", "level"), port("u3", "Boolean")],
             "outputs": [port("y", "Integer", "level")],
             "notes": "The Stage 3 Level 2 shutdown alarm takes priority over the Stage 2 ventilation alarm."},
        ])
        edges.extend({"from": source, "to": target} for source, target in [
            ("actual_supply_air_temp", "freeze_stage_three_below_fifteen.u1"),
            ("freeze_stage_three_fifteen_threshold.y", "freeze_stage_three_below_fifteen.u2"),
            ("actual_supply_air_temp", "freeze_stage_three_below_five.u1"),
            ("freeze_stage_three_five_threshold.y", "freeze_stage_three_below_five.u2"),
            ("freeze_stage_three_below_fifteen.y", "freeze_stage_three_fifteen_delay.u"),
            ("freeze_stage_three_below_five.y", "freeze_stage_three_five_delay.u"),
            ("freeze_stage_three_fifteen_delay.y", "freeze_stage_three_sensor_fault.u1"),
            ("freeze_stage_three_five_delay.y", "freeze_stage_three_sensor_fault.u2"),
            ("freeze_stage_three_sensor_fault.y", "freeze_stage_three_trip.u1"),
            ("freeze_stat_trip", "freeze_stage_three_trip.u2"),
            ("actual_supply_air_temp", "freeze_stage_three_reset_warm.u1"),
            ("freeze_stage_three_reset_temperature.y", "freeze_stage_three_reset_warm.u2"),
            ("freeze_stat_trip", "freeze_stage_three_stat_normal.u"),
            ("freeze_stage_three_reset_warm.y", "freeze_stage_three_reset_safe.u1"),
            ("freeze_stage_three_stat_normal.y", "freeze_stage_three_reset_safe.u2"),
            ("freeze_stage_three_reset_safe.y", "freeze_stage_three_reset_allowed.u1"),
            ("freeze_reset", "freeze_stage_three_reset_allowed.u2"),
            ("freeze_stage_three_trip.y", "freeze_stage_three_latch.u"),
            ("freeze_stage_three_reset_allowed.y", "freeze_stage_three_latch.clr"),
            ("freeze_stage_three_latch.y", "freeze_stage_three"),
            ("freeze_stage_three_fan_off.y", "freeze_stage_three_fan.u1"),
            ("fan_enable.y", "freeze_stage_three_fan.u2"),
            ("freeze_stage_three_latch.y", "freeze_stage_three_fan.u3"),
            ("freeze_stage_three_outdoor_closed.y", "freeze_stage_three_outdoor_air.u1"),
            ("freeze_stage_two_outdoor_air.y", "freeze_stage_three_outdoor_air.u2"),
            ("freeze_stage_three_latch.y", "freeze_stage_three_outdoor_air.u3"),
            ("freeze_stage_three_cooling_open.y", "freeze_stage_three_cooling_valve.u1"),
            ("cooling_coil_enable.y", "freeze_stage_three_cooling_valve.u2"),
            ("freeze_stage_three_latch.y", "freeze_stage_three_cooling_valve.u3"),
            ("actual_supply_air_temp", "freeze_stage_three_measured_high.u1"),
            ("mixed_air_temp", "freeze_stage_three_measured_high.u2"),
            ("freeze_stage_three_heating_target.y", "freeze_stage_three_heating_pi.u_s"),
            ("freeze_stage_three_measured_high.y", "freeze_stage_three_heating_pi.u_m"),
            ("freeze_stage_three_heating_pi.y", "freeze_stage_three_heating_valve.u1"),
            ("heating_coil_enable.y", "freeze_stage_three_heating_valve.u2"),
            ("freeze_stage_three_latch.y", "freeze_stage_three_heating_valve.u3"),
            ("freeze_stage_one_latch.y", "freeze_stage_three_hot_request.u1"),
            ("freeze_stage_three_latch.y", "freeze_stage_three_hot_request.u2"),
            ("freeze_stage_three_hot_request.y", "freeze_stage_one_request_integer.u"),
            ("freeze_stage_three_latch.y", "freeze_chilled_water_request"),
            ("freeze_stage_three_alarm_two.y", "freeze_stage_three_alarm_select.u1"),
            ("freeze_stage_two_alarm_three.y", "freeze_stage_three_alarm_select.u2"),
            ("freeze_stage_three_latch.y", "freeze_stage_three_alarm_select.u3"),
            ("freeze_stage_three_alarm_select.y", "freeze_alarm_level"),
        ])
        replacements = {
            ("fan_enable.y", "fan_speed"): "freeze_stage_three_fan.y",
            ("freeze_stage_two_outdoor_air.y", "outdoor_air_fraction"): "freeze_stage_three_outdoor_air.y",
            ("cooling_coil_enable.y", "cooling_coil_valve_position"): "freeze_stage_three_cooling_valve.y",
            ("heating_coil_enable.y", "heating_coil_valve_position"): "freeze_stage_three_heating_valve.y",
        }
        for edge in edges:
            key = edge["from"], edge["to"]
            if key in replacements:
                edge["from"] = replacements[key]
        edges[:] = [edge for edge in edges if not (
            edge["from"] == "freeze_stage_one_latch.y" and edge["to"] == "freeze_stage_one_request_integer.u")]
        ahu["outputs"].extend([
            port("freeze_stage_three", "Boolean", "unitless", "Freeze Protection Stage 3", "Point"),
            port("freeze_chilled_water_request", "Boolean", "unitless", "Freeze Chilled Water Pump Request", "Point"),
            port("freeze_alarm_level", "Integer", "level", "Freeze Protection Alarm Level", "Point"),
        ])
        graph["outputs"].extend([
            port(f"{name}_freeze_stage_three", "Boolean", "unitless", f"AHU {index} Freeze Protection Stage 3", "Point"),
            port(f"{name}_freeze_chilled_water_request", "Boolean", "unitless", f"AHU {index} Freeze Chilled Water Pump Request", "Point"),
            port(f"{name}_freeze_alarm_level", "Integer", "level", f"AHU {index} Freeze Protection Alarm Level", "Point"),
        ])
        graph["connections"].extend([
            {"from": f"{name}.freeze_stage_three", "to": f"{name}_freeze_stage_three"},
            {"from": f"{name}.freeze_chilled_water_request", "to": f"{name}_freeze_chilled_water_request"},
            {"from": f"{name}.freeze_chilled_water_request", "to": f"plant.{name}_freeze_chilled_water_request"},
            {"from": f"{name}.freeze_alarm_level", "to": f"{name}_freeze_alarm_level"},
        ])
        plant["inputs"].append(port(f"{name}_freeze_chilled_water_request", "Boolean", "unitless",
                                    f"AHU {index} Freeze Chilled Water Pump Request", "Point"))
        ahu["notes"] += " Stage 3 latches fan shutdown, outdoor-air closure, full cooling-coil opening, and heating protection until a safe operator reset."
    plant["blocks"].extend([
        binary("freeze_chilled_water_request_any", "Or", "Boolean", "Boolean",
               "Either AHU shutdown requests chilled-water pump operation."),
        binary("freeze_chilled_water_pump_enable", "Or", "Boolean", "Boolean",
               "Enable the chilled-water pump for ordinary load or AHU freeze shutdown."),
    ])
    plant["connections"] = [edge for edge in plant["connections"] if not (
        edge["from"] == "chilled_water_pump_needed.y" and edge["to"] in
        ("chilled_water_enable", "chilled_water_pump_select.u3", "chilled_reset_enabled.u3"))]
    plant["connections"].extend({"from": source, "to": target} for source, target in [
        ("ahu_1_freeze_chilled_water_request", "freeze_chilled_water_request_any.u1"),
        ("ahu_2_freeze_chilled_water_request", "freeze_chilled_water_request_any.u2"),
        ("chilled_water_pump_needed.y", "freeze_chilled_water_pump_enable.u1"),
        ("freeze_chilled_water_request_any.y", "freeze_chilled_water_pump_enable.u2"),
        ("freeze_chilled_water_pump_enable.y", "chilled_water_enable"),
        ("freeze_chilled_water_pump_enable.y", "chilled_water_pump_select.u3"),
        ("freeze_chilled_water_pump_enable.y", "chilled_reset_enabled.u3"),
    ])
    plant["notes"] += " Stage 3 freeze shutdown energizes chilled-water pumping."


add_ahu_freeze_stage_three()


def add_ahu_economizer_enable():
    """Qualify free cooling with fan proof, freeze state, and dry-bulb hysteresis."""
    for index in (1, 2):
        name = f"ahu_{index}"
        ahu = next(block for block in graph["blocks"] if block["name"] == name)
        graph["inputs"].append(port(f"{name}_supply_fan_proven", "Boolean", "unitless",
                                    f"AHU {index} Supply Fan Proven", "Point",
                                    "Raw proof from delivered fan airflow in the preceding physical step"))
        ahu["inputs"].append(port("supply_fan_proven", "Boolean", "unitless",
                                  "Supply Fan Proven", "Point"))
        graph["connections"].append({"from": f"{name}_supply_fan_proven", "to": f"{name}.supply_fan_proven"})
        ahu["outputs"].append(port("economizer_mode_enabled", "Boolean", "unitless",
                                    "Economizer Mode Enabled", "Point"))
        graph["outputs"].append(port(f"{name}_economizer_mode_enabled", "Boolean", "unitless",
                                      f"AHU {index} Economizer Mode Enabled", "Point"))
        graph["connections"].append({"from": f"{name}.economizer_mode_enabled",
                                     "to": f"{name}_economizer_mode_enabled"})
        ahu["blocks"].extend([
            binary("economizer_high_limit_error", "Subtract", "Real", "Real",
                   "Outdoor temperature minus return temperature for differential dry-bulb high limit."),
            {"name": "economizer_high_limit_hysteresis", "block_type": "Hysteresis", "category": "elementary",
             "inputs": [port("u", unit="K")], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "uLow", "value": -1.0, "unit": "K"},
                            {"name": "uHigh", "value": 0.0, "unit": "K"}],
             "notes": "Disable free cooling when outdoor reaches return temperature; re-enable below return by 1 K."},
            {"name": "economizer_high_limit_ok", "block_type": "Not", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "notes": "Free cooling is available below the differential dry-bulb high limit."},
            binary("economizer_freeze_early", "Or", "Boolean", "Boolean",
                   "Either Stage 1 or Stage 2 freeze protection disables free cooling."),
            binary("economizer_freeze_any", "Or", "Boolean", "Boolean",
                   "Any of the three freeze stages disables free cooling."),
            {"name": "economizer_freeze_clear", "block_type": "Not", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "notes": "Permit economizer control only with no freeze protection active."},
            binary("economizer_proven_and_clear", "And", "Boolean", "Boolean",
                   "Require physical fan proof and no freeze stage."),
        ])
        ahu["connections"] = [edge for edge in ahu["connections"] if
                              (edge["from"], edge["to"]) not in (
                                  ("outdoor_below_return.y", "free_cooling_useful.u1"),
                                  ("fan_requested.y", "economizer_temperature_ok.u2"))]
        ahu["connections"].extend({"from": source, "to": target} for source, target in [
            ("outdoor_c", "economizer_high_limit_error.u1"),
            ("return_air_temp", "economizer_high_limit_error.u2"),
            ("economizer_high_limit_error.y", "economizer_high_limit_hysteresis.u"),
            ("economizer_high_limit_hysteresis.y", "economizer_high_limit_ok.u"),
            ("economizer_high_limit_ok.y", "free_cooling_useful.u1"),
            ("freeze_stage_one_latch.y", "economizer_freeze_early.u1"),
            ("freeze_stage_two_latch.y", "economizer_freeze_early.u2"),
            ("economizer_freeze_early.y", "economizer_freeze_any.u1"),
            ("freeze_stage_three_latch.y", "economizer_freeze_any.u2"),
            ("economizer_freeze_any.y", "economizer_freeze_clear.u"),
            ("supply_fan_proven", "economizer_proven_and_clear.u1"),
            ("economizer_freeze_clear.y", "economizer_proven_and_clear.u2"),
            ("economizer_proven_and_clear.y", "economizer_temperature_ok.u2"),
            ("economizer_enabled.y", "economizer_mode_enabled"),
        ])
        ahu["notes"] += " Differential dry-bulb hysteresis, raw fan proof, and all freeze stages qualify economizer operation."


add_ahu_economizer_enable()


def add_ahu_outdoor_air_limits():
    """Gate the measured-flow minimum OA loop by mode and bound damper travel."""
    for index in (1, 2):
        name = f"ahu_{index}"
        ahu = next(block for block in graph["blocks"] if block["name"] == name)
        graph["connections"].append({"from": "schedule_occupied", "to": f"{name}.occupied_mode"})
        ahu["inputs"].append(port("occupied_mode", "Boolean", "unitless", "Occupied AHU Mode", "Point",
                                  "Editable CDL uses the raw schedule to select minimum outdoor air"))
        ahu["outputs"].append(port("minimum_outdoor_air_enabled", "Boolean", "unitless",
                                    "Minimum Outdoor Air Loop Enabled", "Point"))
        graph["outputs"].append(port(f"{name}_minimum_outdoor_air_enabled", "Boolean", "unitless",
                                      f"AHU {index} Minimum Outdoor Air Loop Enabled", "Point"))
        graph["connections"].append({"from": f"{name}.minimum_outdoor_air_enabled",
                                     "to": f"{name}_minimum_outdoor_air_enabled"})
        ahu["blocks"].extend([
            binary("minimum_oa_mode_enabled", "And", "Boolean", "Boolean",
                   "Enable minimum outdoor-air flow control only in occupied mode with proven fan airflow."),
            constant("minimum_oa_off_setpoint", 0.0, "normalized airflow",
                     "No minimum outdoor airflow is required outside occupied mode or without fan proof."),
            switch("minimum_oa_setpoint", "normalized airflow",
                   "Gate the teaching outdoor-airflow setpoint by occupied mode and fan proof."),
            constant("minimum_oa_off_command", 0.0, "fraction (0–1)",
                     "Disable the minimum outdoor-air PI command when its loop is inactive."),
            switch("minimum_oa_active", "fraction (0–1)",
                   "Use measured-flow minimum damper position only while the loop is enabled."),
            constant("outdoor_air_fan_off", 0.0, "fraction (0–1)",
                     "Close normal outdoor air when the fan is not requested; freeze stages retain priority."),
            switch("outdoor_air_fan_requested", "fraction (0–1)",
                   "Allow normal outside air when the fan is requested, including cooldown and preconditioning."),
            constant("outdoor_damper_physical_max", 1.0, "fraction (0–1)",
                     "Editable mechanical maximum travel of the common outdoor-air damper."),
            binary("outdoor_damper_physical_limit", "Min", "Real", "Real",
                   "Bound the final outdoor damper command by its physical maximum."),
        ])
        replaced = {
            ("minimum_oa_flow_gain.y", "minimum_outdoor_airflow"),
            ("minimum_oa_flow_gain.y", "minimum_oa_pi.u_s"),
            ("minimum_oa_pi.y", "minimum_oa_command.u2"),
            ("minimum_oa_command.y", "freeze_stage_one_outdoor_air.u2"),
            ("freeze_stage_three_outdoor_air.y", "outdoor_air_fraction"),
        }
        ahu["connections"] = [edge for edge in ahu["connections"]
                              if (edge["from"], edge["to"]) not in replaced]
        ahu["connections"].extend({"from": source, "to": target} for source, target in [
            ("occupied_mode", "minimum_oa_mode_enabled.u1"),
            ("supply_fan_proven", "minimum_oa_mode_enabled.u2"),
            ("minimum_oa_mode_enabled.y", "minimum_outdoor_air_enabled"),
            ("minimum_oa_flow_gain.y", "minimum_oa_setpoint.u1"),
            ("minimum_oa_off_setpoint.y", "minimum_oa_setpoint.u2"),
            ("minimum_oa_mode_enabled.y", "minimum_oa_setpoint.u3"),
            ("minimum_oa_setpoint.y", "minimum_outdoor_airflow"),
            ("minimum_oa_setpoint.y", "minimum_oa_pi.u_s"),
            ("minimum_oa_pi.y", "minimum_oa_active.u1"),
            ("minimum_oa_off_command.y", "minimum_oa_active.u2"),
            ("minimum_oa_mode_enabled.y", "minimum_oa_active.u3"),
            ("minimum_oa_active.y", "minimum_oa_command.u2"),
            ("minimum_oa_command.y", "outdoor_air_fan_requested.u1"),
            ("outdoor_air_fan_off.y", "outdoor_air_fan_requested.u2"),
            ("fan_requested.y", "outdoor_air_fan_requested.u3"),
            ("outdoor_air_fan_requested.y", "freeze_stage_one_outdoor_air.u2"),
            ("freeze_stage_three_outdoor_air.y", "outdoor_damper_physical_limit.u1"),
            ("outdoor_damper_physical_max.y", "outdoor_damper_physical_limit.u2"),
            ("outdoor_damper_physical_limit.y", "outdoor_air_fraction"),
        ])
        ahu["notes"] += " Occupied mode and fan proof gate the measured-flow minimum; fan request permits normal economizer air in cooldown and preconditioning. An editable mechanical maximum limits final outdoor-damper travel."


add_ahu_outdoor_air_limits()


def add_ahu_return_damper_control():
    """Expose independent common outdoor/return damper commands to the mixer."""
    for index in (1, 2):
        name = f"ahu_{index}"
        ahu = next(block for block in graph["blocks"] if block["name"] == name)
        for output_name, label in (("outdoor_air_damper_position", "Outdoor Air Damper Command"),
                                   ("return_air_damper_position", "Return Air Damper Command")):
            ahu["outputs"].append(port(output_name, unit="fraction (0–1)", label=label,
                                       brick="Damper_Position_Command"))
            graph["outputs"].append(port(f"{name}_{output_name}", unit="fraction (0–1)",
                                         label=f"AHU {index} {label}", brick="Damper_Position_Command"))
            graph["connections"].append({"from": f"{name}.{output_name}",
                                         "to": f"{name}_{output_name}"})
        ahu["blocks"].extend([
            constant("return_damper_one", 1.0, "fraction (0–1)", "Fully open return damper reference."),
            binary("return_damper_complement", "Subtract", "Real", "Real",
                   "Normal return command complements the outdoor damper command."),
            constant("return_damper_pi_midpoint", 0.5, "fraction (0–1)",
                     "Minimum-OA PI midpoint before return damper begins closing."),
            binary("return_damper_pi_excess", "Subtract", "Real", "Real",
                   "Minimum-OA PI demand above its midpoint."),
            constant("return_damper_zero", 0.0, "fraction (0–1)", "Lower return-damper limit."),
            binary("return_damper_positive_excess", "Max", "Real", "Real",
                   "Ignore PI demand below the return-damper closing range."),
            {"name": "return_damper_double_excess", "block_type": "MultiplyByParameter",
             "category": "elementary", "inputs": [port("u", unit="fraction (0–1)")],
             "outputs": [port("y", unit="fraction (0–1)")],
             "parameters": [{"name": "k", "type": "Real", "value": 2.0, "unit": "unitless"}],
             "notes": "Map the upper half of PI output over the full return damper stroke."},
            binary("return_damper_pi_max", "Subtract", "Real", "Real",
                   "Maximum return opening allowed by minimum outdoor airflow control."),
            binary("return_damper_oa_limited", "Min", "Real", "Real",
                   "Outdoor-air minimum can independently close the return damper."),
            switch("return_damper_mode", "fraction (0–1)",
                   "Apply the minimum-OA return limit only with occupied mode and proven fan."),
            constant("return_damper_physical_max", 1.0, "fraction (0–1)",
                     "Editable mechanical maximum travel of the common return damper."),
            binary("return_damper_physical_limit", "Min", "Real", "Real",
                   "Bound the return damper command by its physical maximum."),
        ])
        ahu["connections"].extend({"from": source, "to": target} for source, target in [
            ("outdoor_damper_physical_limit.y", "outdoor_air_damper_position"),
            ("outdoor_air_select.y", "return_damper_complement.u2"),
            ("return_damper_one.y", "return_damper_complement.u1"),
            ("minimum_oa_pi.y", "return_damper_pi_excess.u1"),
            ("return_damper_pi_midpoint.y", "return_damper_pi_excess.u2"),
            ("return_damper_pi_excess.y", "return_damper_positive_excess.u1"),
            ("return_damper_zero.y", "return_damper_positive_excess.u2"),
            ("return_damper_positive_excess.y", "return_damper_double_excess.u"),
            ("return_damper_one.y", "return_damper_pi_max.u1"),
            ("return_damper_double_excess.y", "return_damper_pi_max.u2"),
            ("return_damper_complement.y", "return_damper_oa_limited.u1"),
            ("return_damper_pi_max.y", "return_damper_oa_limited.u2"),
            ("return_damper_oa_limited.y", "return_damper_mode.u1"),
            ("return_damper_complement.y", "return_damper_mode.u2"),
            ("minimum_oa_mode_enabled.y", "return_damper_mode.u3"),
            ("return_damper_mode.y", "return_damper_physical_limit.u1"),
            ("return_damper_physical_max.y", "return_damper_physical_limit.u2"),
            ("return_damper_physical_limit.y", "return_air_damper_position"),
        ])
        ahu["notes"] += " The return damper has an independent command and mechanical maximum. Its minimum-OA limit closes return air as PI demand rises above 0.5. The mixer derives outside-air share from both actuator openings."


add_ahu_return_damper_control()


def add_ahu_common_damper_minimum_mapping():
    """Map the lower half of minimum-air PI output over outdoor-damper travel."""
    for index in (1, 2):
        ahu = next(block for block in graph["blocks"] if block["name"] == f"ahu_{index}")
        ahu["blocks"].extend([
            binary("minimum_oa_lower_half", "Min", "Real", "Real",
                   "Clip the minimum outdoor-air PI output at the return-damper transition midpoint."),
            {"name": "minimum_oa_outdoor_stroke", "block_type": "MultiplyByParameter",
             "category": "elementary", "inputs": [port("u", unit="fraction (0–1)")],
             "outputs": [port("y", unit="fraction (0–1)")],
             "parameters": [{"name": "k", "type": "Real", "value": 2.0, "unit": "unitless"}],
             "notes": "Map PI output 0–0.5 to outdoor damper minimum position 0–1; above 0.5 the return damper closes instead."},
        ])
        ahu["connections"] = [edge for edge in ahu["connections"]
                              if (edge["from"], edge["to"]) != ("minimum_oa_pi.y", "minimum_oa_active.u1")]
        ahu["connections"].extend({"from": source, "to": target} for source, target in [
            ("minimum_oa_pi.y", "minimum_oa_lower_half.u1"),
            ("return_damper_pi_midpoint.y", "minimum_oa_lower_half.u2"),
            ("minimum_oa_lower_half.y", "minimum_oa_outdoor_stroke.u"),
            ("minimum_oa_outdoor_stroke.y", "minimum_oa_active.u1"),
        ])
        ahu["notes"] += " The first half of minimum-OA PI output opens the outdoor damper from zero to full stroke; the second half limits return-damper opening."


add_ahu_common_damper_minimum_mapping()


def add_ashrae_62_1_ventilation():
    """Calculate zone breathing air and multizone AHU outdoor air in CDL."""
    for index in range(1, 11):
        name = f"zone_{index:02}"
        zone = next(block for block in graph["blocks"] if block["name"] == name)
        graph["inputs"].append(port(f"{name}_occupancy_count", unit="people",
                                    label=f"Zone {index} Occupancy Count Sensor",
                                    brick="Occupancy_Count_Sensor",
                                    description="Physical number of people present in the zone; the control sequence uses this measured count."))
        zone["inputs"].append(port("occupancy_count", unit="people",
                                   label="Occupancy Count Sensor", brick="Occupancy_Count_Sensor"))
        graph["connections"].append({"from": f"{name}_occupancy_count", "to": f"{name}.occupancy_count"})
        if graph.get("brick_model"):
            graph["brick_model"]["entities"].append({
                "id": f"{name}_occupancy_count", "class": "brick:Occupancy_Count_Sensor",
                "label": f"Zone {index:02d} occupancy count", "connector": f"{name}_occupancy_count",
            })
            graph["brick_model"]["relationships"].append({
                "subject": f"hvac_{name}", "predicate": "brick:hasPoint",
                "object": f"{name}_occupancy_count",
            })
        zone["blocks"].extend([
            binary("ventilation_occupied", "And", "Boolean", "Boolean",
                   "Ventilation applies only when the schedule and confirmed occupancy are both active."),
            binary("ventilation_window_eligible", "And", "Boolean", "Boolean",
                   "An open window suppresses mechanical ventilation components for this zone."),
            constant("ventilation_floor_area", 100.0, "m²",
                     "Synthetic design floor area served by this zone, in square metres."),
            constant("ventilation_area_rate", 0.0003, "m³/s/m²",
                     "Editable ASHRAE 62.1 breathing-air design rate per square metre."),
            binary("ventilation_area_design", "Multiply", "Real", "Real",
                   "Design floor area times outdoor-air rate per area."),
            constant("ventilation_person_rate", 0.0025, "m³/s/person",
                     "Editable ASHRAE 62.1 breathing-air design rate per person."),
            binary("ventilation_population_design", "Multiply", "Real", "Real",
                   "Measured occupant count times outdoor-air rate per person."),
            constant("ventilation_zero", 0.0, "normalized airflow", "No breathing-zone air in unoccupied or standby mode."),
            switch("ventilation_area_active", "normalized airflow", "Gate the area component by occupied presence."),
            switch("ventilation_population_active", "normalized airflow", "Gate the population component by occupied presence."),
            binary("ventilation_breathing_sum", "Add", "Real", "Real", "Sum the area and population components."),
            binary("ventilation_heating_supply", "Greater", "Real", "Boolean",
                   "A discharge warmer than the zone uses heating air-distribution effectiveness."),
            constant("ventilation_cooling_effectiveness", 1.0, "unitless", "Cooling air-distribution effectiveness."),
            constant("ventilation_heating_effectiveness", 0.8, "unitless", "Heating air-distribution effectiveness."),
            switch("ventilation_effectiveness", "unitless", "Select heating or cooling distribution effectiveness."),
            binary("ventilation_minimum_oa", "Divide", "Real", "Real",
                   "Breathing-zone outdoor air divided by distribution effectiveness."),
        ])
        design = next(block for block in zone["blocks"] if block["name"] == "ventilation_area_design")
        design["inputs"][0]["unit"], design["inputs"][1]["unit"] = "m²", "m³/s/m²"
        design["outputs"][0]["unit"] = "normalized airflow"
        population = next(block for block in zone["blocks"] if block["name"] == "ventilation_population_design")
        population["inputs"][0]["unit"], population["inputs"][1]["unit"] = "people", "m³/s/person"
        population["outputs"][0]["unit"] = "normalized airflow"
        zone["connections"].extend({"from": source, "to": target} for source, target in [
            ("scheduled_occupied", "ventilation_occupied.u1"),
            ("occupancy_latch.y", "ventilation_occupied.u2"),
            ("ventilation_occupied.y", "ventilation_window_eligible.u1"),
            ("window_closed", "ventilation_window_eligible.u2"),
            ("ventilation_floor_area.y", "ventilation_area_design.u1"),
            ("ventilation_area_rate.y", "ventilation_area_design.u2"),
            ("ventilation_area_design.y", "ventilation_area_active.u1"),
            ("ventilation_zero.y", "ventilation_area_active.u2"),
            ("ventilation_window_eligible.y", "ventilation_area_active.u3"),
            ("occupancy_count", "ventilation_population_design.u1"),
            ("ventilation_person_rate.y", "ventilation_population_design.u2"),
            ("ventilation_population_design.y", "ventilation_population_active.u1"),
            ("ventilation_zero.y", "ventilation_population_active.u2"),
            ("ventilation_window_eligible.y", "ventilation_population_active.u3"),
            ("ventilation_area_active.y", "ventilation_breathing_sum.u1"),
            ("ventilation_population_active.y", "ventilation_breathing_sum.u2"),
            ("discharge_temp", "ventilation_heating_supply.u1"),
            ("room_temp", "ventilation_heating_supply.u2"),
            ("ventilation_heating_effectiveness.y", "ventilation_effectiveness.u1"),
            ("ventilation_cooling_effectiveness.y", "ventilation_effectiveness.u2"),
            ("ventilation_heating_supply.y", "ventilation_effectiveness.u3"),
            ("ventilation_breathing_sum.y", "ventilation_minimum_oa.u1"),
            ("ventilation_effectiveness.y", "ventilation_minimum_oa.u2"),
            ("ventilation_area_active.y", "ventilation_area"),
            ("ventilation_population_active.y", "ventilation_population"),
            ("ventilation_minimum_oa.y", "ventilation_minimum_oa_flow"),
        ])
        for output, label in (("ventilation_area", "Adjusted Area Breathing Air"),
                              ("ventilation_population", "Adjusted Population Breathing Air"),
                              ("ventilation_minimum_oa_flow", "Zone Minimum Outside Air Flow")):
            zone["outputs"].append(port(output, unit="normalized airflow", label=label, brick="Point"))
            graph["outputs"].append(port(f"{name}_{output}", unit="normalized airflow",
                                         label=f"Zone {index} {label}", brick="Point"))
            graph["connections"].append({"from": f"{name}.{output}", "to": f"{name}_{output}"})
        zone["notes"] += " Editable floor area and breathing-air rates combine with measured occupant count. Occupied presence gates area and population ventilation; measured discharge selects heating or cooling distribution effectiveness."

    for ahu_index in (1, 2):
        name = f"ahu_{ahu_index}"
        ahu = next(block for block in graph["blocks"] if block["name"] == name)
        zones = [f"zone_{index:02}" for index in range((ahu_index - 1) * 5 + 1, ahu_index * 5 + 1)]
        for zone_name in zones:
            for suffix in ("ventilation_area", "ventilation_population", "ventilation_minimum_oa_flow", "measured_airflow"):
                input_name = f"{zone_name}_{suffix}"
                ahu["inputs"].append(port(input_name, unit="normalized airflow", label=input_name, brick="Point"))
                source = input_name if suffix == "measured_airflow" else f"{zone_name}.{suffix}"
                graph["connections"].append({"from": source, "to": f"{name}.{input_name}"})
        def sum_five(stem, sources):
            for step in range(4):
                ahu["blocks"].append(binary(f"{stem}_sum_{step+1}", "Add", "Real", "Real",
                                             f"Sum {stem.replace('_',' ')} across the AHU zones."))
                ahu["connections"].extend([
                    {"from": sources[0] if step == 0 else f"{stem}_sum_{step}.y", "to": f"{stem}_sum_{step+1}.u1"},
                    {"from": sources[step+1], "to": f"{stem}_sum_{step+1}.u2"},
                ])
            return f"{stem}_sum_4.y"
        area_sum = sum_five("vent_area", [f"{z}_ventilation_area" for z in zones])
        population_sum = sum_five("vent_population", [f"{z}_ventilation_population" for z in zones])
        primary_sum = sum_five("vent_primary", [f"{z}_measured_airflow" for z in zones])
        for zone_name in zones:
            ahu["blocks"].extend([
                constant(f"{zone_name}_vent_epsilon", 0.0001, "normalized airflow", "Avoid zero primary-flow denominator."),
                binary(f"{zone_name}_vent_primary_safe", "Max", "Real", "Real", "Bound primary-flow denominator."),
                binary(f"{zone_name}_vent_flow_bounded", "Min", "Real", "Real", "Zone outdoor air cannot exceed measured primary flow."),
                binary(f"{zone_name}_vent_fraction", "Divide", "Real", "Real", "Zone outside-air fraction."),
            ])
            ahu["connections"].extend({"from": source, "to": target} for source, target in [
                (f"{zone_name}_measured_airflow", f"{zone_name}_vent_primary_safe.u1"),
                (f"{zone_name}_vent_epsilon.y", f"{zone_name}_vent_primary_safe.u2"),
                (f"{zone_name}_measured_airflow", f"{zone_name}_vent_flow_bounded.u1"),
                (f"{zone_name}_ventilation_minimum_oa_flow", f"{zone_name}_vent_flow_bounded.u2"),
                (f"{zone_name}_vent_flow_bounded.y", f"{zone_name}_vent_fraction.u1"),
                (f"{zone_name}_vent_primary_safe.y", f"{zone_name}_vent_fraction.u2"),
            ])
        fraction_max = [f"{z}_vent_fraction.y" for z in zones]
        for step in range(4):
            ahu["blocks"].append(binary(f"vent_fraction_max_{step+1}", "Max", "Real", "Real",
                                         "Maximum occupied-zone outside-air fraction."))
            ahu["connections"].extend([
                {"from": fraction_max[0] if step == 0 else f"vent_fraction_max_{step}.y", "to": f"vent_fraction_max_{step+1}.u1"},
                {"from": fraction_max[step+1], "to": f"vent_fraction_max_{step+1}.u2"},
            ])
        ahu["blocks"].extend([
            binary("vent_breathing_total", "Add", "Real", "Real", "Sum adjusted area and population ventilation."),
            constant("vent_uncorrected_design", 0.45, "normalized airflow", "Editable design uncorrected minimum outside-air flow for five zones."),
            binary("vent_uncorrected_limit", "Min", "Real", "Real", "Cap uncorrected airflow by design."),
            constant("vent_primary_epsilon", 0.00045, "normalized airflow", "Avoid zero system primary-flow denominator."),
            binary("vent_primary_safe", "Max", "Real", "Real", "Bound system primary-flow denominator."),
            binary("vent_uncorrected_fraction", "Divide", "Real", "Real", "Uncorrected outdoor air divided by measured primary air."),
            constant("vent_one", 1.0, "unitless", "Reference ventilation efficiency of one."),
            binary("vent_efficiency_base", "Add", "Real", "Real", "One plus the uncorrected system outside-air fraction."),
            binary("vent_efficiency", "Subtract", "Real", "Real", "Subtract the maximum zone outside-air fraction."),
            constant("vent_efficiency_epsilon", 0.0001, "unitless", "Lower numerical bound for system ventilation efficiency."),
            binary("vent_efficiency_safe", "Max", "Real", "Real", "Avoid division by zero at low ventilation efficiency."),
            binary("vent_effective_flow", "Divide", "Real", "Real", "Correct uncorrected outdoor air for system ventilation efficiency."),
            constant("vent_total_design", 0.75, "normalized airflow", "Editable design total outside-air flow cap."),
            binary("vent_effective_limit", "Min", "Real", "Real", "Cap effective outside-air flow by design."),
        ])
        ahu["connections"].extend({"from": source, "to": target} for source, target in [
            (area_sum, "vent_breathing_total.u1"),
            (population_sum, "vent_breathing_total.u2"),
            ("vent_breathing_total.y", "vent_uncorrected_limit.u1"),
            ("vent_uncorrected_design.y", "vent_uncorrected_limit.u2"),
            (primary_sum, "vent_primary_safe.u1"),
            ("vent_primary_epsilon.y", "vent_primary_safe.u2"),
            ("vent_uncorrected_limit.y", "vent_uncorrected_fraction.u1"),
            ("vent_primary_safe.y", "vent_uncorrected_fraction.u2"),
            ("vent_one.y", "vent_efficiency_base.u1"),
            ("vent_uncorrected_fraction.y", "vent_efficiency_base.u2"),
            ("vent_efficiency_base.y", "vent_efficiency.u1"),
            ("vent_fraction_max_4.y", "vent_efficiency.u2"),
            ("vent_efficiency.y", "vent_efficiency_safe.u1"),
            ("vent_efficiency_epsilon.y", "vent_efficiency_safe.u2"),
            ("vent_uncorrected_limit.y", "vent_effective_flow.u1"),
            ("vent_efficiency_safe.y", "vent_effective_flow.u2"),
            ("vent_effective_flow.y", "vent_effective_limit.u1"),
            ("vent_total_design.y", "vent_effective_limit.u2"),
            ("vent_effective_limit.y", "minimum_oa_setpoint.u1"),
            ("vent_uncorrected_limit.y", "ventilation_uncorrected"),
            ("vent_efficiency_safe.y", "ventilation_efficiency"),
        ])
        ahu["connections"] = [edge for edge in ahu["connections"]
                              if (edge["from"], edge["to"]) != ("minimum_oa_flow_gain.y", "minimum_oa_setpoint.u1")]
        ahu["blocks"] = [block for block in ahu["blocks"] if block["name"] != "minimum_oa_flow_gain"]
        ahu["connections"] = [edge for edge in ahu["connections"]
                              if not edge["from"].startswith("minimum_oa_flow_gain.")
                              and not edge["to"].startswith("minimum_oa_flow_gain.")]
        for output, label, unit in (("ventilation_uncorrected", "Uncorrected Outside Air Setpoint", "normalized airflow"),
                                    ("ventilation_efficiency", "System Ventilation Efficiency", "unitless")):
            ahu["outputs"].append(port(output, unit=unit, label=label, brick="Point"))
            graph["outputs"].append(port(f"{name}_{output}", unit=unit, label=f"AHU {ahu_index} {label}", brick="Point"))
            graph["connections"].append({"from": f"{name}.{output}", "to": f"{name}_{output}"})
        ahu["notes"] += " Editable zone area/population components, measured primary flow, zone outdoor fractions, and system ventilation efficiency determine the minimum outside-air setpoint."


add_ashrae_62_1_ventilation()


def add_zone_co2_reset():
    """Use measured zone CO2 to reset occupied VAV minimum flow in CDL."""
    for index in range(1, 11):
        name = f"zone_{index:02}"
        input_name = f"{name}_co2_ppm"
        zone = next(block for block in graph["blocks"] if block["name"] == name)
        sensor = port(input_name, unit="ppm", label=f"Zone {index} CO₂ Level Sensor",
                      brick="Zone_CO2_Level_Sensor",
                      description="Physical zone CO₂ concentration from the mixed-air mass balance.")
        graph["inputs"].append(sensor)
        zone["inputs"].append(port("co2_ppm", unit="ppm", label="Zone CO₂ Level Sensor",
                                   brick="Zone_CO2_Level_Sensor"))
        graph["connections"].append({"from": input_name, "to": f"{name}.co2_ppm"})
        zone["blocks"].extend([
            constant("co2_reset_lower", 700.0, "ppm", "Start increasing occupied minimum airflow 200 ppm below the CO₂ setpoint."),
            constant("co2_reset_span", 200.0, "ppm", "Editable CO₂ proportional reset span."),
            binary("co2_above_lower", "Subtract", "Real", "Real", "Measured CO₂ above the reset start."),
            binary("co2_reset_ratio", "Divide", "Real", "Real", "Normalize the CO₂ error by the editable span."),
            {"name": "co2_reset_limit", "block_type": "Limiter", "category": "elementary",
             "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", unit="fraction (0–1)")],
             "parameters": [{"name": "uMin", "value": 0.0}, {"name": "uMax", "value": 1.0}],
             "notes": "Match the OBC CO₂ reset line from setpoint minus 200 ppm to setpoint."},
            constant("co2_cooling_max_flow", 0.95, "normalized airflow", "Teaching design cooling maximum for this VAV."),
            binary("co2_available_flow", "Subtract", "Real", "Real", "Cooling maximum minus the current occupied minimum."),
            binary("co2_flow_increment", "Multiply", "Real", "Real", "Scale available flow by CO₂ reset fraction."),
            binary("co2_flow_reset", "Add", "Real", "Real", "Raise occupied minimum airflow toward cooling maximum."),
            binary("co2_population_component", "Multiply", "Real", "Real",
                   "Reset the measured-count population ventilation component from zero to design as CO₂ rises."),
            switch("co2_minimum_flow", "normalized airflow", "Apply the CO₂ reset only with scheduled, confirmed occupancy."),
            switch("window_primary_minimum", "normalized airflow",
                   "Suppress the occupied primary-air minimum when the window contact is open; freeze heating remains a separate request."),
        ])
        population_reset = next(block for block in zone["blocks"] if block["name"] == "co2_population_component")
        population_reset["inputs"][0]["unit"] = "normalized airflow"
        population_reset["inputs"][1]["unit"] = "fraction (0–1)"
        population_reset["outputs"][0]["unit"] = "normalized airflow"
        edges = zone["connections"]
        for edge in edges:
            if edge["from"] == "minimum_airflow_heating.y" and edge["to"] in ("airflow_sum.u1", "airflow_request_sum.u1"):
                edge["from"] = "window_primary_minimum.y"
            if edge["from"] == "ventilation_population_design.y" and edge["to"] == "ventilation_population_active.u1":
                edge["from"] = "co2_population_component.y"
        edges.extend({"from": source, "to": target} for source, target in [
            ("co2_ppm", "co2_above_lower.u1"),
            ("co2_reset_lower.y", "co2_above_lower.u2"),
            ("co2_above_lower.y", "co2_reset_ratio.u1"),
            ("co2_reset_span.y", "co2_reset_ratio.u2"),
            ("co2_reset_ratio.y", "co2_reset_limit.u"),
            ("ventilation_population_design.y", "co2_population_component.u1"),
            ("co2_reset_limit.y", "co2_population_component.u2"),
            ("co2_cooling_max_flow.y", "co2_available_flow.u1"),
            ("minimum_airflow_heating.y", "co2_available_flow.u2"),
            ("co2_available_flow.y", "co2_flow_increment.u1"),
            ("co2_reset_limit.y", "co2_flow_increment.u2"),
            ("minimum_airflow_heating.y", "co2_flow_reset.u1"),
            ("co2_flow_increment.y", "co2_flow_reset.u2"),
            ("co2_flow_reset.y", "co2_minimum_flow.u1"),
            ("minimum_airflow_heating.y", "co2_minimum_flow.u2"),
            ("ventilation_occupied.y", "co2_minimum_flow.u3"),
            ("co2_minimum_flow.y", "window_primary_minimum.u1"),
            ("ventilation_zero.y", "window_primary_minimum.u2"),
            ("window_closed", "window_primary_minimum.u3"),
        ])
        zone["notes"] += " Measured CO₂ resets occupied minimum primary airflow through editable CDL; the physical model only supplies concentration."
        if graph.get("brick_model"):
            graph["brick_model"]["entities"].append({
                "id": input_name, "class": "brick:Zone_CO2_Level_Sensor",
                "label": f"Zone {index:02d} CO₂ level", "connector": input_name,
            })
            graph["brick_model"]["relationships"].append({
                "subject": f"hvac_{name}", "predicate": "brick:hasPoint", "object": input_name,
            })


add_zone_co2_reset()


def add_zone_occupied_standby_ventilation():
    """Let the editable standby policy decide whether vacancy suppresses area air and VAV minimum."""
    for index in range(1, 11):
        zone = next(block for block in graph["blocks"] if block["name"] == f"zone_{index:02}")
        zone["blocks"].extend([
            {"name": "ventilation_standby_permitted", "block_type": "Buildings.Controls.OBC.CDL.Logical.Sources.Constant",
             "category": "elementary", "inputs": [], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "k", "type": "Boolean", "value": True}],
             "notes": "When true, confirmed vacancy permits occupied standby with zero area air and minimum primary flow. Set false where occupied standby is not permitted."},
            {"name": "ventilation_standby_disabled", "block_type": "Not", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "notes": "Retain occupied area air and minimum primary flow during vacancy when standby is disallowed."},
            binary("ventilation_area_presence_or_required", "Or", "Boolean", "Boolean",
                   "Area ventilation stays on for confirmed presence or when occupied standby is disallowed."),
            binary("ventilation_area_mode_eligible", "And", "Boolean", "Boolean",
                   "Require local occupied mode for area air and occupied minimum flow."),
            binary("ventilation_area_window_eligible", "And", "Boolean", "Boolean",
                   "An open window suppresses area air and occupied minimum flow regardless of standby policy."),
        ])
        replaced = {("ventilation_window_eligible.y", "ventilation_area_active.u3"),
                    ("window_closed", "window_primary_minimum.u3")}
        standby_targets = ("occupied_setpoint_requested.u1", "mode_select_occupied.u2",
                           "minimum_damper_standby.u3", "minimum_airflow_standby.u3")
        replaced.update(("occupancy_latch.y", target) for target in standby_targets)
        zone["connections"] = [edge for edge in zone["connections"]
                               if (edge["from"], edge["to"]) not in replaced]
        zone["connections"].extend({"from": source, "to": target} for source, target in [
            ("ventilation_standby_permitted.y", "ventilation_standby_disabled.u"),
            ("occupancy_latch.y", "ventilation_area_presence_or_required.u1"),
            ("ventilation_standby_disabled.y", "ventilation_area_presence_or_required.u2"),
            *(("ventilation_area_presence_or_required.y", target) for target in standby_targets),
            ("local_occupied.y", "ventilation_area_mode_eligible.u1"),
            ("ventilation_area_presence_or_required.y", "ventilation_area_mode_eligible.u2"),
            ("ventilation_area_mode_eligible.y", "ventilation_area_window_eligible.u1"),
            ("window_closed", "ventilation_area_window_eligible.u2"),
            ("ventilation_area_window_eligible.y", "ventilation_area_active.u3"),
            ("ventilation_area_window_eligible.y", "window_primary_minimum.u3"),
        ])
        zone["notes"] += " An editable occupied-standby policy retains area ventilation and base primary minimum during vacancy where standby is disallowed; population air still follows confirmed presence."


def add_water_source_flow_proof():
    """Keep source production off until graph logic proves loop water flow."""
    plant = next(block for block in graph["blocks"] if block["name"] == "plant")
    for loop, command_pairs in (
        ("hot", (("heat_pump_heating", "heat_pump_heating_kw"), ("boiler", "boiler_kw"))),
        ("chilled", (("heat_pump_cooling", "heat_pump_cooling_kw"), ("chiller", "chiller_kw"))),
    ):
        water = "hot_water" if loop == "hot" else "chilled_water"
        enabled = f"freeze_{water}_pump_enable.y"
        flow = f"{water}_flow_kg_s"
        plant["blocks"].extend([
            constant(f"{loop}_source_flow_threshold", 0.1, "kg/s",
                     "Minimum measured loop flow before source equipment may produce heat transfer."),
            binary(f"{loop}_source_flow_present", "Greater", "Real", "Boolean",
                   "Prove measured water flow from the previous physical step."),
            binary(f"{loop}_source_proven", "And", "Boolean", "Boolean",
                   "Source production requires both the CDL pump enable and measured water flow."),
            constant(f"{loop}_source_zero", 0.0, "kW", "Zero source command before pump flow is proven."),
        ])
        plant["connections"].extend({"from": source, "to": target} for source, target in [
            (flow, f"{loop}_source_flow_present.u1"),
            (f"{loop}_source_flow_threshold.y", f"{loop}_source_flow_present.u2"),
            (enabled, f"{loop}_source_proven.u1"),
            (f"{loop}_source_flow_present.y", f"{loop}_source_proven.u2"),
            (f"{loop}_source_proven.y", f"{loop}_source_proven_output"),
        ])
        plant["outputs"].append(port(f"{loop}_source_proven_output", "Boolean", "unitless",
                                     f"{loop.title()} Water Source Flow Proven", "Point"))
        graph["outputs"].append(port(f"plant_{loop}_source_proven", "Boolean", "unitless",
                                     f"{loop.title()} Water Source Flow Proven", "Point"))
        graph["connections"].append({"from": f"plant.{loop}_source_proven_output",
                                     "to": f"plant_{loop}_source_proven"})
        for command, output in command_pairs:
            gate = f"{command}_flow_gate"
            plant["blocks"].append(switch(gate, "kW", "Disable source production until pump flow is proven."))
            plant["connections"] = [edge for edge in plant["connections"]
                                    if (edge["from"], edge["to"]) != (f"{command}.y", output)]
            plant["connections"].extend({"from": source, "to": target} for source, target in [
                (f"{command}.y", f"{gate}.u1"),
                (f"{loop}_source_zero.y", f"{gate}.u2"),
                (f"{loop}_source_proven.y", f"{gate}.u3"),
                (f"{gate}.y", output),
            ])
    plant["notes"] += " Measured loop water flow and graph pump enable prove source operation before heat-pump, boiler, or chiller commands are issued."


add_water_source_flow_proof()


def add_water_reset_stage_hold():
    """Hold sampled plant resets after source-stage changes until flow is stable."""
    plant = next(block for block in graph["blocks"] if block["name"] == "plant")
    for loop, sources, water in (
        ("hot", ("heat_pump_heating_flow_gate", "boiler_flow_gate"), "hot_water"),
        ("chilled", ("heat_pump_cooling_flow_gate", "chiller_flow_gate"), "chilled_water"),
    ):
        reset = f"{loop}_reset"
        enable = f"freeze_{water}_pump_enable.y"
        blocks = plant["blocks"]
        edges = plant["connections"]
        blocks.extend([
            constant(f"{loop}_stage_on_threshold", 0.1, "kW",
                     "Count a source as staged on once its CDL command exceeds 0.1 kW."),
            binary(f"{loop}_stage_hp_active", "Greater", note="Heat-pump source stage is commanded on."),
            binary(f"{loop}_stage_backup_active", "Greater", note="Boiler or chiller backup stage is commanded on."),
            constant(f"{loop}_stage_zero", 0.0, "count", "No active source stage."),
            constant(f"{loop}_stage_hp_code", 1.0, "count", "Code for the active heat-pump source stage."),
            constant(f"{loop}_stage_backup_code", 2.0, "count", "Code for the active backup source stage."),
            switch(f"{loop}_stage_hp_select", "count", "Encode heat-pump source status as zero or one."),
            switch(f"{loop}_stage_backup_select", "count", "Encode backup source status as zero or two."),
            binary(f"{loop}_stage_code", "Add", "Real", "Real", "Sum stage codes to distinguish all four source combinations."),
            {"name": f"{loop}_stage_observed", "block_type": "UnitDelay", "category": "elementary",
             "inputs": [port("u", unit="count")], "outputs": [port("y", unit="count")],
             "parameters": [{"name": "samplePeriod", "value": 60.0, "unit": "s"},
                            {"name": "y_start", "value": 0.0, "unit": "count"}],
             "notes": "Observe the previous step's source command without feeding the reset target back into its own stage detector."},
            {"name": f"{loop}_stage_previous", "block_type": "UnitDelay", "category": "elementary",
             "inputs": [port("u", unit="count")], "outputs": [port("y", unit="count")],
             "parameters": [{"name": "samplePeriod", "value": 60.0, "unit": "s"},
                            {"name": "y_start", "value": 0.0, "unit": "count"}],
             "notes": "Hold the prior source-stage code for one internal minute so CDL can detect changes."},
            binary(f"{loop}_stage_raised", "Greater", note="A source stage was added since the prior sample."),
            binary(f"{loop}_stage_lowered", "Less", note="A source stage was removed since the prior sample."),
            binary(f"{loop}_stage_changed", "Or", "Boolean", "Boolean",
                   "Detect any source-stage transition."),
            {"name": f"{loop}_stage_unchanged", "block_type": "Not", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "notes": "The graph source-stage combination matches the prior sample."},
            binary(f"{loop}_stage_stable_and_proven", "And", "Boolean", "Boolean",
                   "Require a stable stage combination and measured water-flow proof."),
            {"name": f"{loop}_stage_hold_delay", "block_type": "TrueDelay", "category": "elementary",
             "inputs": [port("u", "Boolean")], "outputs": [port("y", "Boolean")],
             "parameters": [{"name": "delayTime", "value": 900.0, "unit": "s"},
                            {"name": "delayOnInit", "type": "Boolean", "value": True}],
             "notes": "Hold reset for at least 900 seconds after the source stage changes or flow proof is lost."},
            switch(f"{reset}_stage_hold", "fraction (0–1)",
                   "Use the prior sampled reset until the source stage is proven and stable for 15 minutes."),
            {"name": f"{reset}_held_value", "block_type": "UnitDelay", "category": "elementary",
             "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", unit="fraction (0–1)")],
             "parameters": [{"name": "samplePeriod", "value": 60.0, "unit": "s"},
                            {"name": "y_start", "value": 1.0 if loop == "chilled" else 1.0 / 6.0}],
             "notes": "Remember the exposed reset at the last internal step so a stage transition cannot advance a queued five-minute sample."},
            switch(f"{reset}_output_select", "fraction (0–1)",
                   "Expose the sampled reset when ready; otherwise expose the held value."),
            switch(f"{reset}_output_active", "fraction (0–1)",
                   "Restore the initial reset while this water loop is disabled."),
        ])
        edges[:] = [edge for edge in edges if not (
            edge["to"] == f"{reset}_enabled.u1" and edge["from"] == f"{reset}_limit.y"
            or edge["to"] == f"{reset}_enabled.u3"
            or edge["from"] == f"{reset}_previous.y" and edge["to"] in {
                f"{water}_reset_fraction", "chilled_reset_temperature_excess.u1", "hot_reset_inverse.u2"})]
        edges.extend({"from": source, "to": target} for source, target in [
            (f"{sources[0]}.y", f"{loop}_stage_hp_active.u1"),
            (f"{sources[1]}.y", f"{loop}_stage_backup_active.u1"),
            (f"{loop}_stage_on_threshold.y", f"{loop}_stage_hp_active.u2"),
            (f"{loop}_stage_on_threshold.y", f"{loop}_stage_backup_active.u2"),
            (f"{loop}_stage_hp_code.y", f"{loop}_stage_hp_select.u1"),
            (f"{loop}_stage_zero.y", f"{loop}_stage_hp_select.u2"),
            (f"{loop}_stage_hp_active.y", f"{loop}_stage_hp_select.u3"),
            (f"{loop}_stage_backup_code.y", f"{loop}_stage_backup_select.u1"),
            (f"{loop}_stage_zero.y", f"{loop}_stage_backup_select.u2"),
            (f"{loop}_stage_backup_active.y", f"{loop}_stage_backup_select.u3"),
            (f"{loop}_stage_hp_select.y", f"{loop}_stage_code.u1"),
            (f"{loop}_stage_backup_select.y", f"{loop}_stage_code.u2"),
            (f"{loop}_stage_code.y", f"{loop}_stage_observed.u"),
            (f"{loop}_stage_observed.y", f"{loop}_stage_previous.u"),
            (f"{loop}_stage_observed.y", f"{loop}_stage_raised.u1"),
            (f"{loop}_stage_previous.y", f"{loop}_stage_raised.u2"),
            (f"{loop}_stage_observed.y", f"{loop}_stage_lowered.u1"),
            (f"{loop}_stage_previous.y", f"{loop}_stage_lowered.u2"),
            (f"{loop}_stage_raised.y", f"{loop}_stage_changed.u1"),
            (f"{loop}_stage_lowered.y", f"{loop}_stage_changed.u2"),
            (f"{loop}_stage_changed.y", f"{loop}_stage_unchanged.u"),
            (f"{loop}_stage_unchanged.y", f"{loop}_stage_stable_and_proven.u1"),
            (f"{loop}_source_proven.y", f"{loop}_stage_stable_and_proven.u2"),
            (f"{loop}_stage_stable_and_proven.y", f"{loop}_stage_hold_delay.u"),
            (f"{reset}_limit.y", f"{reset}_stage_hold.u1"),
            (f"{reset}_held_value.y", f"{reset}_stage_hold.u2"),
            (f"{loop}_stage_hold_delay.y", f"{reset}_stage_hold.u3"),
            (f"{reset}_stage_hold.y", f"{reset}_enabled.u1"),
            (enable, f"{reset}_enabled.u3"),
            (f"{reset}_previous.y", f"{reset}_output_select.u1"),
            (f"{reset}_held_value.y", f"{reset}_output_select.u2"),
            (f"{loop}_stage_hold_delay.y", f"{reset}_output_select.u3"),
            (f"{reset}_output_select.y", f"{reset}_output_active.u1"),
            (f"{reset}_initial.y", f"{reset}_output_active.u2"),
            (enable, f"{reset}_output_active.u3"),
            (f"{reset}_output_active.y", f"{reset}_held_value.u"),
            (f"{reset}_output_active.y", f"{water}_reset_fraction"),
            (f"{reset}_output_active.y", "chilled_reset_temperature_excess.u1" if loop == "chilled" else "hot_reset_inverse.u2"),
        ])
    plant["notes"] += " CDL detects heat-pump and backup source-stage changes, then holds both sampled water resets until measured flow and the selected stage remain stable for 900 seconds."


add_water_reset_stage_hold()


def add_water_differential_pressure_control():
    """Reset remote water-pressure targets and control pumps from measured DP."""
    plant = next(block for block in graph["blocks"] if block["name"] == "plant")
    for loop, reset in (("hot", "hot_reset_output_active"), ("chilled", "chilled_reset_output_active")):
        water = "hot_water" if loop == "hot" else "chilled_water"
        sensor = f"{water}_differential_pressure_pa"
        speed_output = f"{water}_pump_speed"
        enable = f"freeze_{water}_pump_enable.y"
        graph["inputs"].append(port(sensor, unit="Pa", label=f"{loop.title()} Water Remote Differential Pressure Sensor",
                                    brick="Differential_Pressure_Sensor",
                                    description="Previous physical step's remote differential pressure across the water-load branches"))
        plant["inputs"].append(port(sensor, unit="Pa", label=f"{loop.title()} Water Remote Differential Pressure Sensor",
                                   brick="Differential_Pressure_Sensor"))
        graph["connections"].append({"from": sensor, "to": f"plant.{sensor}"})
        plant["blocks"].extend([
            constant(f"{loop}_dp_half_reset", 0.5, "fraction (0–1)",
                     "Upper limit of the pressure-reset half of the sampled plant reset."),
            binary(f"{loop}_dp_reset_limited", "Min", "Real", "Real",
                   "Use only the first half of the sampled plant reset for pressure."),
            {"name": f"{loop}_dp_reset_span", "block_type": "MultiplyByParameter", "category": "elementary",
             "inputs": [port("u", unit="fraction (0–1)")], "outputs": [port("y", unit="Pa")],
             "parameters": [{"name": "k", "type": "Real", "value": 40_000.0, "unit": "Pa"}],
             "notes": "Map reset 0–0.5 to an editable 0–20 kPa pressure rise."},
            constant(f"{loop}_dp_minimum", 15_000.0, "Pa",
                     "Editable minimum remote pressure for this simplified water network."),
            binary(f"{loop}_dp_target_sum", "Add", "Real", "Real",
                   "Pressure target from minimum and sampled reset allowance."),
            constant(f"{loop}_dp_zero", 0.0, "Pa", "No pressure target when the loop is disabled."),
            switch(f"{loop}_dp_target_active", "Pa", "Enable the pressure target only while the water loop is requested."),
            {"name": f"{loop}_dp_pi", "block_type": "LimPID", "category": "elementary",
             "inputs": [port("u_s", unit="Pa"), port("u_m", unit="Pa")],
             "outputs": [port("y", unit="fraction (0–1)")],
             "parameters": [{"name": "k", "type": "Real", "value": 0.000005, "unit": "Pa⁻¹"},
                            {"name": "Ti", "type": "Real", "value": 600.0, "unit": "s"},
                            {"name": "yMin", "type": "Real", "value": 0.0},
                            {"name": "yMax", "type": "Real", "value": 1.0},
                            {"name": "reverseActing", "type": "Boolean", "value": True}],
             "notes": "Editable water pump PI compares active remote pressure target with previous-step measured pressure."},
            constant(f"{loop}_dp_pump_minimum", 0.15, "fraction (0–1)",
                     "Minimum running pump speed for source proof and circulation."),
            binary(f"{loop}_dp_speed_minimum", "Max", "Real", "Real",
                   "Maintain minimum pump speed when the pressure PI is below its lower bound."),
        ])
        plant["connections"] = [edge for edge in plant["connections"]
                                if (edge["from"], edge["to"]) != (f"{water}_pump_limit.y", f"{water}_pump_select.u1")]
        plant["connections"].extend({"from": source, "to": target} for source, target in [
            (f"{reset}.y", f"{loop}_dp_reset_limited.u1"),
            (f"{loop}_dp_half_reset.y", f"{loop}_dp_reset_limited.u2"),
            (f"{loop}_dp_reset_limited.y", f"{loop}_dp_reset_span.u"),
            (f"{loop}_dp_minimum.y", f"{loop}_dp_target_sum.u1"),
            (f"{loop}_dp_reset_span.y", f"{loop}_dp_target_sum.u2"),
            (f"{loop}_dp_target_sum.y", f"{loop}_dp_target_active.u1"),
            (f"{loop}_dp_zero.y", f"{loop}_dp_target_active.u2"),
            (enable, f"{loop}_dp_target_active.u3"),
            (f"{loop}_dp_target_active.y", f"{loop}_dp_pi.u_s"),
            (sensor, f"{loop}_dp_pi.u_m"),
            (f"{loop}_dp_pi.y", f"{loop}_dp_speed_minimum.u1"),
            (f"{loop}_dp_pump_minimum.y", f"{loop}_dp_speed_minimum.u2"),
            (f"{loop}_dp_speed_minimum.y", f"{water}_pump_select.u1"),
            (f"{loop}_dp_target_active.y", f"{loop}_dp_setpoint_pa"),
        ])
        plant["outputs"].append(port(f"{loop}_dp_setpoint_pa", unit="Pa",
                                     label=f"{loop.title()} Water Remote Differential Pressure Setpoint",
                                     brick="Differential_Pressure_Setpoint"))
        graph["outputs"].append(port(f"plant_{loop}_dp_setpoint_pa", unit="Pa",
                                     label=f"{loop.title()} Water Remote Differential Pressure Setpoint",
                                     brick="Differential_Pressure_Setpoint"))
        graph["connections"].append({"from": f"plant.{loop}_dp_setpoint_pa",
                                     "to": f"plant_{loop}_dp_setpoint_pa"})
        retired = {f"{water}_pump_gain", f"{water}_pump_limit"}
        plant["blocks"] = [block for block in plant["blocks"] if block["name"] not in retired]
        plant["connections"] = [edge for edge in plant["connections"]
                                if edge["from"].split(".")[0] not in retired
                                and edge["to"].split(".")[0] not in retired]
    plant["notes"] += " Sampled reset values set remote hot/chilled differential-pressure targets; editable CDL PI loops use measured pressure to command pump speed."


add_water_differential_pressure_control()


def remove_vav_cold_supply_startup_interlock():
    """Keep a zone cooling request available while the AHU coil starts up."""
    for zone in (block for block in graph["blocks"] if block["block_type"] == "VAVBoxController"):
        zone["blocks"] = [block for block in zone["blocks"]
                          if block["name"] not in ("air_can_cool", "cooling_enabled")]
        zone["connections"] = [edge for edge in zone["connections"]
                               if edge["from"].split(".")[0] not in ("air_can_cool", "cooling_enabled")
                               and edge["to"].split(".")[0] not in ("air_can_cool", "cooling_enabled")]
        zone["connections"].append({"from": "cooling_mode.y", "to": "damper_select.u3"})
        zone["notes"] += " A cooling call can open its damper while supply air is warm during AHU startup; a warm supply is diagnosed at the AHU rather than suppressing the request."


remove_vav_cold_supply_startup_interlock()
add_equipment_availability_control()
add_physical_source_stage_feedback()
add_heat_pump_start_fault_control()
add_backup_start_fault_control("boiler")
add_backup_start_fault_control("chiller")
add_zone_conditioning_modes(graph, port, binary)
add_zone_groups(graph, port, constant, binary)
add_group_unoccupied_modes(graph, port, constant, binary, switch)
add_window_control(graph, port, constant, binary, switch)
add_group_overrides(graph, port, binary)
add_learned_optimal_start(graph, port, constant, binary, switch)
add_obc_zone_status_to_groups(graph, binary)
add_obc_operation_mode_to_groups(graph, port)
add_g36_supply_temperature_modes(graph, port, constant, binary, switch)
add_g36_obc_supply_air_reset(graph, port, constant)
add_obc_group_status_to_groups(graph)
add_terminal_airflow_alarms(graph, port, constant, binary)
add_pressure_request_levels(graph, port)
add_sat_request_levels(graph, port)
add_sat_dynamic_suppression(graph, port)
add_terminal_hot_request_levels(graph, port)
add_ahu_plant_requests(graph, port)
add_airflow_sensor_alarms(graph, port, constant, binary)
add_low_discharge_alarms(graph, port, constant, binary)
add_terminal_leak_alarms(graph, port, constant, binary)
add_supply_fan_alarms(graph, port, constant, binary)
add_fan_speed_feedback(graph, port, constant, binary)
add_ahu_damper_feedback(graph, port, constant, binary)
add_economizer_high_limit_hold(graph, port)
add_fixed_dry_bulb_high_limit(graph, port, constant, binary)
add_economizer_disable_transition(graph, port, binary, switch)
add_zone_occupied_standby_ventilation()
integrate_obc_optimal_start(graph)
(EXAMPLES / "four_pipe_g36.json").write_text(json.dumps(graph, indent=2) + "\n")
