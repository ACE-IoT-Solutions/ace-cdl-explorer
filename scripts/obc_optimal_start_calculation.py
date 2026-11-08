#!/usr/bin/env python3
"""Build the pinned OBC OptimalStartCalculation wiring as editable CDL."""

import argparse
import hashlib
import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "verification/g36_optimal_start_calculation_modelica.mo"
SOURCE_SHA256 = "b3afc892b070d712ee116f330ae1d41b8737c1dad58c45634dd0ba61f4086c0a"

PORTS = {
    "Logical.Sources.SampleTrigger": ((), (("y", "Boolean"),)),
    "Discrete.TriggeredMovingMean": ((("u", "Real"), ("trigger", "Boolean")), (("y", "Real"),)),
    "Discrete.TriggeredSampler": ((("u", "Real"), ("trigger", "Boolean")), (("y", "Real"),)),
    "Reals.Sources.Constant": ((), (("y", "Real"),)),
    "Logical.Edge": ((("u", "Boolean"),), (("y", "Boolean"),)),
    "Logical.FallingEdge": ((("u", "Boolean"),), (("y", "Boolean"),)),
    "Reals.Hysteresis": ((("u", "Real"),), (("y", "Boolean"),)),
    "Logical.TrueFalseHold": ((("u", "Boolean"),), (("y", "Boolean"),)),
    "Reals.Divide": ((("u1", "Real"), ("u2", "Real")), (("y", "Real"),)),
    "Logical.Pre": ((("u", "Boolean"),), (("y", "Boolean"),)),
    "Reals.Subtract": ((("u1", "Real"), ("u2", "Real")), (("y", "Real"),)),
    "Reals.Min": ((("u1", "Real"), ("u2", "Real")), (("y", "Real"),)),
    "Logical.Latch": ((("u", "Boolean"), ("clr", "Boolean")), (("y", "Boolean"),)),
    "Logical.Timer": ((("u", "Boolean"),), (("y", "Real"), ("passed", "Boolean"))),
    "Logical.Not": ((("u", "Boolean"),), (("y", "Boolean"),)),
    "Reals.GreaterThreshold": ((("u", "Real"),), (("y", "Boolean"),)),
    "Logical.And": ((("u1", "Boolean"), ("u2", "Boolean")), (("y", "Boolean"),)),
    "Reals.Max": ((("u1", "Real"), ("u2", "Real")), (("y", "Real"),)),
    "Reals.Switch": ((("u1", "Real"), ("u2", "Boolean"), ("u3", "Real")), (("y", "Real"),)),
}
INPUTS = {"TDif": "Real", "staCal": "Boolean", "tNexOcc": "Real"}
OUTPUTS = {"tOpt": "Real", "optOn": "Boolean"}


def optimal_start_calculation(t_opt_max=10800.0, n_day=3, u_low=0.0,
                              u_high=0.5, threshold_on=60.0):
    if hashlib.sha256(SOURCE.read_bytes()).hexdigest() != SOURCE_SHA256:
        raise ValueError("pinned OBC OptimalStartCalculation source changed")
    source = SOURCE.read_text()
    declaration, equation = re.split(r"(?m)^equation\s*$", source, maxsplit=1)
    found = list(re.finditer(
        r"(?m)^  Buildings\.Controls\.OBC\.CDL\.([\w.]+)\s+(\w+)\b",
        declaration))
    parameters = {
        "samTri": {"period": 86400.0, "shift": 0.0},
        "samTemSloAve": {"n": n_day},
        "con": {"k": 0.0},
        "timReaMin": {"k": 120.0},
        "defTemSlo": {"k": 1 / 3600},
        "hys": {"uLow": u_low, "uHigh": u_high},
        "hysOpt": {"pre_y_start": False, "uHigh": 0.0, "uLow": -60.0},
        "maxStaTim": {"k": t_opt_max},
        "truHol": {"falseHoldDuration": 0.0,
                   "trueHoldDuration": t_opt_max + 11 * 3600},
        "greThr": {"t": threshold_on},
        "reqStaUp": {"t": 0.0, "h": 0.0},
    }
    blocks = []
    for match in found:
        kind, name = match.groups()
        if kind.startswith("Interfaces."):
            continue
        if kind not in PORTS:
            raise ValueError(f"unsupported OBC OptimalStartCalculation block {kind}")
        incoming, outgoing = PORTS[kind]
        blocks.append({
            "name": name, "block_type": f"Buildings.Controls.OBC.CDL.{kind}",
            "category": "elementary",
            "inputs": [{"name": port, "type": typ} for port, typ in incoming],
            "outputs": [{"name": port, "type": typ} for port, typ in outgoing],
            "parameters": [{"name": key, "value": value,
                            "type": "Boolean" if isinstance(value, bool) else
                                    "Integer" if isinstance(value, int) else "Real"}
                           for key, value in parameters.get(name, {}).items()],
        })
    sources = set(INPUTS)
    targets = set(OUTPUTS)
    for block in blocks:
        sources.update(f'{block["name"]}.{port["name"]}' for port in block["outputs"])
        targets.update(f'{block["name"]}.{port["name"]}' for port in block["inputs"])
    edges = []
    for source_port, target_port in re.findall(
            r"connect\(\s*([\w. ]+)\s*,\s*([\w. ]+)\s*\)", equation):
        start = re.sub(r"\s+", "", source_port)
        end = re.sub(r"\s+", "", target_port)
        if start in targets and end in sources:
            start, end = end, start
        if start not in sources or end not in targets:
            raise ValueError(f"unresolved OBC OptimalStartCalculation wire {start} -> {end}")
        edges.append({"from": start, "to": end})
    if len(blocks) != 32 or len(edges) != 47:
        raise ValueError(f"OBC OptimalStartCalculation extraction changed: {len(blocks)} blocks, {len(edges)} wires")
    return {
        "name": "OBCOptimalStartCalculation", "block_type": "OptimalStartCalculation",
        "category": "composite",
        "description": "Pinned OBC optimal-start learning and request calculation.",
        "notes": "The OBC Logical.Pre feedback requires Boolean event iteration before physical time advances. Parameters use the OBC OptimalStart wrapper defaults.",
        "inputs": [{"name": name, "type": typ,
                    **({"unit": "s"} if name == "tNexOcc" else {"unit": "K"} if name == "TDif" else {})}
                   for name, typ in INPUTS.items()],
        "outputs": [{"name": name, "type": typ,
                     **({"unit": "s"} if name == "tOpt" else {})}
                    for name, typ in OUTPUTS.items()],
        "blocks": blocks, "connections": edges,
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    options = parser.parse_args()
    target = ROOT / "web/public/examples/obc_optimal_start_calculation.json"
    content = json.dumps(optimal_start_calculation(), indent=2) + "\n"
    if options.check:
        if not target.exists() or target.read_text() != content:
            raise ValueError(f"OBC OptimalStartCalculation graph drift: {target}")
    else:
        target.write_text(content)
