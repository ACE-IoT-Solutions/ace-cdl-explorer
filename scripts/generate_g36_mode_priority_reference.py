#!/usr/bin/env python3
"""Verify the shipped editable mode arbiter over every request combination."""

import copy
import csv
import io
import itertools
import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VERIFY = ROOT / "verification"
SOURCE = ROOT / "web/public/examples/four_pipe_g36.json"
BOUNDARY = {
    "obc_group_status.uGroOcc": "occupied",
    "obc_operation_mode.yCoolDownRequest": "cooldown_request",
    "obc_operation_mode.yWarmUpRequest": "warmup_request",
    "obc_operation_mode.ySetBackRequest": "setback_request",
    "obc_operation_mode.ySetUpRequest": "setup_request",
    "obc_operation_mode.yFreezeProtectionRequest": "freeze_request",
    "warmup_longer.y": "warmup_longer",
    "lookahead_allowed": "lookahead_allowed",
}
INPUTS = tuple(BOUNDARY.values())
MODES = {
    "occupied": 1, "cooldown": 2, "setup": 3, "warmup": 4,
    "setback": 5, "freeze": 6, "unoccupied": 7,
}


def extract(group: dict) -> dict:
    blocks = {block["name"]: block for block in group["blocks"]}
    drivers: dict[str, str] = {}
    for edge in group["connections"]:
        if edge["to"] in drivers:
            raise ValueError(f"{group['name']}: two drivers for {edge['to']}")
        drivers[edge["to"]] = edge["from"]
    if drivers.get("operation_mode") != "mode_select_occupied.y":
        raise ValueError(f"{group['name']}: output bypasses the CDL mode arbiter")
    included: dict[str, dict] = {}
    edges: list[dict[str, str]] = []
    used_inputs: set[str] = set()

    def visit(name: str) -> None:
        if name in included:
            return
        if name not in blocks or not name.startswith("mode_"):
            raise ValueError(f"{group['name']}: unexpected mode source {name}")
        block = blocks[name]
        for port in block["inputs"]:
            target = f"{name}.{port['name']}"
            source = drivers.get(target)
            if source is None:
                raise ValueError(f"{group['name']}: missing driver for {target}")
            source_block = source.split(".", 1)[0]
            if source_block.startswith("mode_"):
                visit(source_block)
                edges.append({"from": source, "to": target})
            elif source in BOUNDARY:
                alias = BOUNDARY[source]
                if port["type"] != "Boolean":
                    raise ValueError(f"{group['name']}: boundary {source} changed type")
                used_inputs.add(alias)
                edges.append({"from": alias, "to": target})
            else:
                raise ValueError(f"{group['name']}: unexpected mode input {source}")
        included[name] = copy.deepcopy(block)

    visit("mode_select_occupied")
    if used_inputs != set(INPUTS):
        raise ValueError(f"{group['name']}: mode arbiter boundary changed")
    edges.append({"from": "mode_select_occupied.y", "to": "operation_mode"})
    return {
        "name": "G36ModePriority", "block_type": "ModePriority",
        "category": "composite",
        "description": "Actual editable CDL arbiter extracted from a shipped G36 zone group.",
        "inputs": [{"name": name, "type": "Boolean"} for name in INPUTS],
        "outputs": [{"name": "operation_mode", "type": "Integer"}],
        "blocks": list(included.values()), "connections": edges,
    }


def expected(values: dict[str, bool]) -> int:
    if values["occupied"]:
        return MODES["occupied"]
    warmup = values["warmup_request"] and (
        not values["cooldown_request"] or values["warmup_longer"]
    )
    cooldown = values["cooldown_request"] and not warmup
    if values["lookahead_allowed"] and cooldown:
        return MODES["cooldown"]
    if values["lookahead_allowed"] and warmup:
        return MODES["warmup"]
    if values["freeze_request"]:
        return MODES["freeze"]
    if values["setback_request"]:
        return MODES["setback"]
    if values["setup_request"]:
        return MODES["setup"]
    return MODES["unoccupied"]


def trend() -> str:
    with io.StringIO(newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(("time_s", *INPUTS, "y"))
        for index, bits in enumerate(itertools.product((False, True), repeat=len(INPUTS))):
            values = dict(zip(INPUTS, bits))
            writer.writerow((index * 15, *(int(bit) for bit in bits), expected(values)))
        return handle.getvalue()


def main() -> None:
    if sys.argv[1:] not in ([], ["--check"]):
        raise SystemExit("usage: generate_g36_mode_priority_reference.py [--check]")
    building = json.loads(SOURCE.read_text(encoding="utf-8"))
    building_wires = {(edge["from"], edge["to"])
                      for edge in building["connections"]}
    for index in (1, 2):
        source = f"zone_group_{index}.operation_mode"
        for target in (f"ahu_{index}.operation_mode",
                       f"ahu_{index}_group_operation_mode"):
            if (source, target) not in building_wires:
                raise ValueError(f"building mode output is not connected: {source} -> {target}")
    groups = [next(block for block in building["blocks"] if block["name"] == name)
              for name in ("zone_group_1", "zone_group_2")]
    graph = extract(groups[0])
    if graph != extract(groups[1]):
        raise ValueError("the two shipped G36 groups have different mode arbiters")
    files = {
        VERIFY / "g36_mode_priority_validation.json": json.dumps(graph, indent=2) + "\n",
        VERIFY / "g36_mode_priority.csv": trend(),
    }
    for target, content in files.items():
        if sys.argv[1:] == ["--check"]:
            if target.read_text(encoding="utf-8") != content:
                raise ValueError(f"mode priority fixture drift: {target}")
        else:
            target.write_text(content, encoding="utf-8", newline="")


if __name__ == "__main__":
    main()
