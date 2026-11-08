"""Build editable CDL from pinned OBC G36 ZoneGroups.OperationMode wiring.

The pinned Modelica source is vendored under verification/g36_operation_mode_modelica.mo
from modelica-buildings d3d8e6f76514dfbc7d65574c6b5a777b686fe1b7.
The two Utilities.Assert blocks produce messages rather than control outputs and
are omitted here; their conditions remain available as editable CDL signals.
"""

import argparse
import hashlib
import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "verification/g36_operation_mode_modelica.mo"
SOURCE_SHA256 = "37081c76f1be0a88ca6e4c9768ab27fbd889feec7355ba30e7dfb3a575f60301"
MODES = {
    "coolDown": 2, "freezeProtection": 6, "occupied": 1,
    "setBack": 5, "setUp": 3, "unoccupied": 7, "warmUp": 4,
}
PORTS = {
    "Reals.Sources.Constant": ((), (("y", "Real"),)),
    "Integers.Sources.Constant": ((), (("y", "Integer"),)),
    "Reals.Switch": ((("u1", "Real"), ("u2", "Boolean"), ("u3", "Real")), (("y", "Real"),)),
    "Integers.GreaterThreshold": ((("u", "Integer"),), (("y", "Boolean"),)),
    "Integers.Equal": ((("u1", "Integer"), ("u2", "Integer")), (("y", "Boolean"),)),
    "Integers.Add": ((("u1", "Integer"), ("u2", "Integer")), (("y", "Integer"),)),
    "Logical.Latch": ((("u", "Boolean"), ("clr", "Boolean")), (("y", "Boolean"),)),
    "Logical.Edge": ((("u", "Boolean"),), (("y", "Boolean"),)),
    "Logical.TrueFalseHold": ((("u", "Boolean"),), (("y", "Boolean"),)),
    "Logical.Not": ((("u", "Boolean"),), (("y", "Boolean"),)),
    "Logical.And": ((("u1", "Boolean"), ("u2", "Boolean")), (("y", "Boolean"),)),
    "Logical.Or": ((("u1", "Boolean"), ("u2", "Boolean")), (("y", "Boolean"),)),
    "Reals.Hysteresis": ((("u", "Real"),), (("y", "Boolean"),)),
    "Reals.Subtract": ((("u1", "Real"), ("u2", "Real")), (("y", "Real"),)),
    "Conversions.RealToInteger": ((("u", "Real"),), (("y", "Integer"),)),
    "Conversions.BooleanToInteger": ((("u", "Boolean"),), (("y", "Integer"),)),
    "Conversions.BooleanToReal": ((("u", "Boolean"),), (("y", "Real"),)),
}
INPUTS = {
    "u1Occ": "Boolean", "tNexOcc": "Real", "maxCooDowTim": "Real",
    "u1HigOccCoo": "Boolean", "maxWarUpTim": "Real", "u1OccHeaHig": "Boolean",
    "uOpeWin": "Integer", "totColZon": "Integer", "u1SetBac": "Boolean",
    "u1EndSetBac": "Boolean", "TZonMin": "Real", "totHotZon": "Integer",
    "u1SetUp": "Boolean", "u1EndSetUp": "Boolean",
}
PARAMETERS = {"k", "t", "uLow", "uHigh", "pre_y_start", "trueHoldDuration",
              "realTrue", "realFalse", "integerTrue", "integerFalse"}


def _value(text, n_zones, max_recovery_s, freeze_on_k, freeze_off_k):
    value = text.strip()
    if value == "true":
        return True
    if value == "false":
        return False
    if value == "nZon":
        return n_zones
    if value == "preWarCooTim":
        return max_recovery_s
    if value == "TZonFreProOn":
        return freeze_on_k
    if value == "TZonFreProOff":
        return freeze_off_k
    if value.startswith("Buildings.Controls.OBC.ASHRAE.G36.Types.OperationModes."):
        return MODES[value.rsplit(".", 1)[-1]]
    return float(value) if "." in value else int(value)


def operation_mode(n_zones=1, max_recovery_s=10800, freeze_on_k=277.15,
                   freeze_off_k=280.15):
    raw = SOURCE.read_bytes()
    if hashlib.sha256(raw).hexdigest() != SOURCE_SHA256:
        raise ValueError("pinned OBC OperationMode source changed")
    source = raw.decode()
    declaration, equation = re.split(r"(?m)^equation\s*$", source, maxsplit=1)
    pattern = re.compile(r"(?m)^  Buildings\.Controls\.OBC\.CDL\.([\w.]+)\s+(\w+)\b")
    found = list(pattern.finditer(declaration))
    blocks = []
    skipped = set()
    for index, match in enumerate(found):
        kind, name = match.groups()
        if kind.startswith("Interfaces."):
            continue
        if kind == "Utilities.Assert":
            skipped.add(name)
            continue
        if kind not in PORTS:
            raise ValueError(f"unsupported pinned OBC OperationMode block: {kind} {name}")
        snippet = declaration[match.end():found[index + 1].start() if index + 1 < len(found) else len(declaration)]
        settings = snippet.split("annotation", 1)[0]
        params = []
        for key, raw in re.findall(r"\b(\w+)\s*=\s*([\w.\-]+)", settings):
            if key in PARAMETERS:
                value = _value(raw, n_zones, max_recovery_s, freeze_on_k, freeze_off_k)
                params.append({"name": key, "value": value, "type": "Boolean" if isinstance(value, bool) else "Integer" if isinstance(value, int) else "Real"})
        inputs, outputs = PORTS[kind]
        blocks.append({
            "name": name,
            "block_type": f"Buildings.Controls.OBC.CDL.{kind}",
            "category": "elementary",
            "inputs": [{"name": port, "type": typ} for port, typ in inputs],
            "outputs": [{"name": port, "type": typ} for port, typ in outputs],
            "parameters": params,
        })
    sources = set(INPUTS)
    targets = {"yOpeMod"}
    for block in blocks:
        sources.update(f'{block["name"]}.{port["name"]}' for port in block["outputs"])
        targets.update(f'{block["name"]}.{port["name"]}' for port in block["inputs"])
    edges = []
    for source_port, target_port in re.findall(r"connect\(\s*([\w. ]+)\s*,\s*([\w. ]+)\s*\)", equation):
        start = re.sub(r"\s+", "", source_port)
        end = re.sub(r"\s+", "", target_port)
        if start.split(".")[0] in skipped or end.split(".")[0] in skipped:
            continue
        if start in targets and end in sources:
            start, end = end, start
        if start not in sources or end not in targets:
            raise ValueError(f"unresolved pinned OBC connection: {start} -> {end}")
        edges.append({"from": start, "to": end})
    if len(blocks) != 78 or len(edges) != 122:
        raise ValueError(f"pinned OperationMode extraction changed: {len(blocks)} blocks, {len(edges)} wires")
    decision_ports = {
        "yCoolDownRequest": "and4.y",
        "yWarmUpRequest": "and1.y",
        "ySetBackRequest": "lat.y",
        "yFreezeProtectionRequest": "lat1.y",
        "ySetUpRequest": "lat2.y",
    }
    edges.extend({"from": source, "to": output} for output, source in decision_ports.items())
    return {
        "name": "OBCOperationMode", "block_type": "OperationMode", "category": "composite",
        "description": "Pinned OBC G36 zone-group operation mode selected by editable CDL blocks.",
        "notes": "Uses OBC mode codes 1 occupied, 2 cool-down, 3 set-up, 4 warm-up, 5 set-back, 6 freeze protection, 7 unoccupied. Message-only Assert blocks are represented separately as alarms in the building example.",
        "inputs": [{"name": name, "type": typ, **({"unit": "s"} if name in ("tNexOcc", "maxCooDowTim", "maxWarUpTim") else {"unit": "K"} if name == "TZonMin" else {})}
                   for name, typ in INPUTS.items()],
        "outputs": [{"name": "yOpeMod", "type": "Integer"}] +
                   [{"name": name, "type": "Boolean"} for name in decision_ports],
        "blocks": blocks, "connections": edges,
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    options = parser.parse_args()
    target = ROOT / "web/public/examples/obc_operation_mode.json"
    content = json.dumps(operation_mode(), indent=2) + "\n"
    if options.check:
        if not target.exists() or target.read_text() != content:
            raise ValueError(f"OBC OperationMode graph drift: {target}")
    else:
        target.write_text(content)
