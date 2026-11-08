#!/usr/bin/env python3
"""Replace the teaching G36 zone recovery learners with pinned OBC CDL."""

import argparse
import copy
import json
from pathlib import Path

from obc_optimal_start_calculation import optimal_start_calculation


ROOT = Path(__file__).resolve().parents[1]
TARGET = ROOT / "web/public/examples/four_pipe_g36.json"


def port(name, kind, unit=None):
    value = {"name": name, "type": kind}
    if unit:
        value["unit"] = unit
    return value


def elementary(name, kind, inputs, outputs, parameters=None, notes=""):
    return {
        "name": name,
        "block_type": f"Buildings.Controls.OBC.CDL.{kind}",
        "category": "elementary",
        "inputs": inputs,
        "outputs": outputs,
        "parameters": parameters or [],
        "notes": notes,
    }


def wire(source, target):
    return {"from": source, "to": target}


def upgrade_group(group, reference):
    if any(block["name"] == "optimal_warmup_1_obc" for block in group["blocks"]):
        return
    old = {
        block["name"] for block in group["blocks"]
        if block["name"].startswith("optimal_")
        and not block["name"].startswith(("optimal_warmup_longest_", "optimal_cooldown_longest_"))
    }
    old.update({"warmup_seconds_per_k", "cooldown_seconds_per_k"})
    group["blocks"] = [block for block in group["blocks"] if block["name"] not in old]
    group["connections"] = [edge for edge in group["connections"]
                            if edge["from"].split(".")[0] not in old
                            and edge["to"].split(".")[0] not in old]
    group["blocks"].extend([
        elementary("optimal_start_horizon_offset", "Reals.AddParameter",
                   [port("u", "Real", "s")], [port("y", "Real", "s")],
                   [{"name": "p", "value": -10800.0, "type": "Real", "unit": "s"}],
                   "Offset the raw next-occupancy time by the OBC three-hour maximum start."),
        elementary("optimal_start_horizon_state", "Reals.GreaterThreshold",
                   [port("u", "Real", "s")], [port("y", "Boolean")],
                   [{"name": "t", "value": 60.0, "type": "Real", "unit": "s"},
                    {"name": "h", "value": 60.0, "type": "Real", "unit": "s"}],
                   "Pinned OBC wrapper hysteresis around the maximum-start boundary."),
        elementary("optimal_start_sample_event", "Logical.FallingEdge",
                   [port("u", "Boolean")], [port("y", "Boolean")],
                   notes="Pulse at the three-hour lookahead crossing to sample each zone's error."),
    ])
    group["connections"].extend([
        wire("seconds_until_occupied", "optimal_start_horizon_offset.u"),
        wire("optimal_start_horizon_offset.y", "optimal_start_horizon_state.u"),
        wire("optimal_start_horizon_state.y", "optimal_start_sample_event.u"),
    ])
    for direction, error_name, status_port in (
        ("warmup", "warmup_error", "warUpTim"),
        ("cooldown", "cooldown_error", "cooDowTim"),
    ):
        for zone in range(1, 6):
            name = f"optimal_{direction}_{zone}_obc"
            child = copy.deepcopy(reference)
            child["name"] = name
            child["description"] = f"Pinned OBC {direction} optimal-start calculation for zone {zone}."
            group["blocks"].append(child)
            group["connections"].extend([
                wire(f"{error_name}_{zone}.y", f"{name}.TDif"),
                wire("optimal_start_sample_event.y", f"{name}.staCal"),
                wire("seconds_until_occupied", f"{name}.tNexOcc"),
                wire(f"{name}.tOpt", f"zone_status_{zone}.{status_port}"),
            ])
    group["notes"] = (group.get("notes", "") + " Each zone's warm-up and cooldown time now comes from the pinned OBC OptimalStartCalculation CDL graph. The group mode and window filter remain editable CDL decisions.").strip()
    names = {block["name"] for block in group["blocks"]}
    for edge in group["connections"]:
        for endpoint in (edge["from"], edge["to"]):
            block_name = endpoint.split(".")[0]
            if "." in endpoint and block_name not in names:
                raise ValueError(f"unresolved group connection {endpoint}")


def check_group(group, reference):
    names = {block["name"] for block in group["blocks"]}
    if any((name.startswith("optimal_")
            and not name.startswith(("optimal_warmup_longest_", "optimal_cooldown_longest_", "optimal_start_"))
            and not name.endswith("_obc"))
           or name in {"warmup_seconds_per_k", "cooldown_seconds_per_k"}
           for name in names):
        raise ValueError(f"{group['name']}: obsolete recovery prediction remains")
    wires = {(edge["from"], edge["to"]) for edge in group["connections"]}
    destinations = [edge["to"] for edge in group["connections"]]
    if len(destinations) != len(set(destinations)):
        raise ValueError(f"{group['name']}: duplicate connection destination")
    expected_wires = {
        ("seconds_until_occupied", "optimal_start_horizon_offset.u"),
        ("optimal_start_horizon_offset.y", "optimal_start_horizon_state.u"),
        ("optimal_start_horizon_state.y", "optimal_start_sample_event.u"),
    }
    for direction, error_name, status_port in (
        ("warmup", "warmup_error", "warUpTim"),
        ("cooldown", "cooldown_error", "cooDowTim"),
    ):
        for zone in range(1, 6):
            name = f"optimal_{direction}_{zone}_obc"
            expected_wires.update({
                (f"{error_name}_{zone}.y", f"{name}.TDif"),
                ("optimal_start_sample_event.y", f"{name}.staCal"),
                ("seconds_until_occupied", f"{name}.tNexOcc"),
                (f"{name}.tOpt", f"zone_status_{zone}.{status_port}"),
            })
    missing = expected_wires - wires
    if missing:
        raise ValueError(f"{group['name']}: missing OBC optimal-start wires {sorted(missing)}")
    embedded = [block for block in group["blocks"] if block["name"].endswith("_obc") and block["name"].startswith("optimal_")]
    if len(embedded) != 10:
        raise ValueError(f"{group['name']}: expected ten OBC optimal-start instances")
    for block in embedded:
        expected = copy.deepcopy(reference)
        expected["name"] = block["name"]
        expected["description"] = block["description"]
        if block != expected:
            raise ValueError(f"{group['name']}.{block['name']}: pinned OBC graph drift")


def integrate_graph(graph):
    """Integrate and verify OBC optimal start in a generated or saved sample graph."""
    reference = optimal_start_calculation()
    groups = [block for block in graph["blocks"] if block["name"].startswith("zone_group_")]
    if len(groups) != 2:
        raise ValueError("expected two G36 zone groups")
    for group in groups:
        upgrade_group(group, reference)
        check_group(group, reference)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    options = parser.parse_args()
    graph = json.loads(TARGET.read_text())
    before = json.dumps(graph, sort_keys=True)
    integrate_graph(graph)
    after = json.dumps(graph, sort_keys=True)
    if options.check:
        if before != after:
            raise ValueError("bundled G36 graph lacks pinned OBC optimal-start integration")
    elif before != after:
        TARGET.write_text(json.dumps(graph, indent=2) + "\n")


if __name__ == "__main__":
    main()
