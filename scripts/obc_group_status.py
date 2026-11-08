#!/usr/bin/env python3
"""Expand pinned OBC G36 GroupStatus vector operations into editable CDL blocks.

Vector filters are resolved at generation time. Multi-input reductions become
chains of standard two-input CDL blocks, preserving the source equations.
"""

import argparse
import hashlib
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "verification/g36_group_status_modelica.mo"
SOURCE_SHA256 = "246e73aca6f39a150ab95603cba43fca8d022820f0d6cf7549ed64d446626160"


def group_status(zones=5, building_zones=None):
    if hashlib.sha256(SOURCE.read_bytes()).hexdigest() != SOURCE_SHA256:
        raise ValueError("pinned OBC GroupStatus source changed")
    if zones < 1 or (building_zones is not None and building_zones < zones):
        raise ValueError("GroupStatus requires 1..nBuiZon included zones")
    building_zones = building_zones or zones
    blocks, connections, inputs, outputs = [], [], [], []

    def port(name, typ, unit=None):
        return {"name": name, "type": typ, **({"unit": unit} if unit else {})}

    def block(name, kind, incoming, typ, parameters=None):
        blocks.append({
            "name": name, "block_type": f"Buildings.Controls.OBC.CDL.{kind}",
            "category": "elementary", "inputs": [port(*p) for p in incoming],
            "outputs": [port("y", typ)],
            "parameters": [{"name": key, "value": value,
                            "type": "Boolean" if isinstance(value, bool) else
                                    "Integer" if isinstance(value, int) else "Real"}
                           for key, value in (parameters or {}).items()],
        })
        return f"{name}.y"

    def wire(source, target):
        connections.append({"from": source, "to": target})

    def unary(name, kind, source, in_type, out_type, parameters=None):
        result = block(name, kind, [("u", in_type)], out_type, parameters)
        wire(source, f"{name}.u")
        return result

    def binary(name, kind, left, right, left_type, right_type, out_type):
        result = block(name, kind, [("u1", left_type), ("u2", right_type)], out_type)
        wire(left, f"{name}.u1")
        wire(right, f"{name}.u2")
        return result

    def fold(name, kind, sources, typ, result_type=None):
        result = sources[0]
        for index, source in enumerate(sources[1:], 2):
            result = binary(f"{name}_{index}", kind, result, source,
                            typ, typ, result_type or typ)
        return result

    vector_types = {
        "zonOcc": ("Boolean", None), "u1Occ": ("Boolean", None),
        "tNexOcc": ("Real", "s"), "uCooTim": ("Real", "s"),
        "uWarTim": ("Real", "s"), "u1OccHeaHig": ("Boolean", None),
        "u1HigOccCoo": ("Boolean", None), "u1UnoHeaHig": ("Boolean", None),
        "THeaSetOff": ("Real", "K"), "u1EndSetBac": ("Boolean", None),
        "u1HigUnoCoo": ("Boolean", None), "TCooSetOff": ("Real", "K"),
        "u1EndSetUp": ("Boolean", None), "TZon": ("Real", "K"),
        "u1Win": ("Boolean", None),
    }
    for name, (typ, unit) in vector_types.items():
        for index in range(1, zones + 1):
            inputs.append(port(f"{name}_{index}", typ, unit))

    def vector(name):
        return [f"{name}_{index}" for index in range(1, zones + 1)]

    def output(name, typ, source, unit=None):
        outputs.append(port(name, typ, unit))
        wire(source, name)

    scheduled = fold("scheduled_occupied", "Logical.Or", vector("u1Occ"), "Boolean")
    override = fold("override_occupied", "Logical.Or", vector("zonOcc"), "Boolean")
    output("uGroOcc", "Boolean", binary("group_occupied", "Logical.Or",
                                       scheduled, override, "Boolean", "Boolean", "Boolean"))
    upcoming = fold("next_occupancy_min", "Reals.Min", vector("tNexOcc"), "Real")
    allowed = unary("next_occupancy_allowed", "Conversions.BooleanToReal", scheduled,
                    "Boolean", "Real", {"realTrue": 0.0, "realFalse": 1.0})
    output("nexOcc", "Real", binary("next_occupancy", "Reals.Multiply", upcoming,
                                    allowed, "Real", "Real", "Real"), "s")
    output("yCooTim", "Real", fold("cooldown_longest", "Reals.Max",
                                   vector("uCooTim"), "Real"), "s")
    output("yWarTim", "Real", fold("warmup_longest", "Reals.Max",
                                   vector("uWarTim"), "Real"), "s")
    output("yOccHeaHig", "Boolean", fold("any_occupied_heat_error", "Logical.Or",
                                         vector("u1OccHeaHig"), "Boolean"))
    output("yHigOccCoo", "Boolean", fold("any_occupied_cool_error", "Logical.Or",
                                         vector("u1HigOccCoo"), "Boolean"))

    def count(name, source):
        integers = [unary(f"{name}_integer_{index}", "Conversions.BooleanToInteger",
                          point, "Boolean", "Integer")
                    for index, point in enumerate(source, 1)]
        return fold(name, "Integers.Add", integers, "Integer")

    output("yColZon", "Integer", count("cold_zones", vector("u1UnoHeaHig")))
    output("yHotZon", "Integer", count("hot_zones", vector("u1HigUnoCoo")))
    output("yEndSetBac", "Boolean", fold("all_setback_finished", "Logical.And",
                                         vector("u1EndSetBac"), "Boolean"))
    output("yEndSetUp", "Boolean", fold("all_setup_finished", "Logical.And",
                                        vector("u1EndSetUp"), "Boolean"))
    output("TZonMax", "Real", fold("maximum_zone_temperature", "Reals.Max",
                                   vector("TZon"), "Real"), "K")
    output("TZonMin", "Real", fold("minimum_zone_temperature", "Reals.Min",
                                   vector("TZon"), "Real"), "K")
    open_contacts = [unary(f"window_open_{index}", "Logical.Not", point,
                           "Boolean", "Boolean")
                     for index, point in enumerate(vector("u1Win"), 1)]
    output("yOpeWin", "Integer", count("open_windows", open_contacts))

    zone_sum = fold("zone_temperature_sum", "Reals.Add", vector("TZon"), "Real")
    heat_sum = fold("setback_target_sum", "Reals.Add", vector("THeaSetOff"), "Real")
    cool_sum = fold("setup_target_sum", "Reals.Add", vector("TCooSetOff"), "Real")
    denominator_int = block("building_zone_count", "Integers.Sources.Constant", [],
                            "Integer", {"k": building_zones})
    denominator = unary("building_zone_count_real", "Conversions.IntegerToReal",
                        denominator_int, "Integer", "Real")
    heat_delta = binary("setback_temperature_delta", "Reals.Subtract", heat_sum,
                        zone_sum, "Real", "Real", "Real")
    cool_delta = binary("setup_temperature_delta", "Reals.Subtract", zone_sum,
                        cool_sum, "Real", "Real", "Real")
    for name, delta in (("ySetBac", heat_delta), ("ySetUp", cool_delta)):
        average = binary(f"{name}_average", "Reals.Divide", delta,
                         denominator, "Real", "Real", "Real")
        decision = unary(f"{name}_hysteresis", "Reals.Hysteresis", average,
                         "Real", "Boolean", {"uLow": -0.1, "uHigh": 0.1,
                                             "pre_y_start": False})
        output(name, "Boolean", decision)

    return {
        "name": "OBCGroupStatus", "block_type": "GroupStatus", "category": "composite",
        "description": "OBC G36 group status expanded into editable CDL reductions.",
        "notes": "Pinned GroupStatus.mo. Scalar ports represent included zones after the OBC vector mask. The reference divides aggregate temperature error by nBuiZon, even when the group mask includes fewer zones.",
        "inputs": inputs, "outputs": outputs, "blocks": blocks,
        "connections": connections,
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    options = parser.parse_args()
    target = ROOT / "web/public/examples/obc_group_status.json"
    content = json.dumps(group_status(), indent=2) + "\n"
    if options.check:
        if not target.exists() or target.read_text() != content:
            raise ValueError(f"OBC GroupStatus graph drift: {target}")
    else:
        target.write_text(content)
