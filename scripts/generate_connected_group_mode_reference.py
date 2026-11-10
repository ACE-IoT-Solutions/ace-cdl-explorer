#!/usr/bin/env python3
"""Build a connected, editable GroupStatus -> OperationMode verification case."""

import csv
import hashlib
import io
import json
import sys
from pathlib import Path

from obc_group_status import group_status
from obc_operation_mode import operation_mode


ROOT = Path(__file__).resolve().parents[1]
VERIFY = ROOT / "verification"
PINNED = {
    "g36_connected_group_mode_modelica.mo": "54753d7dbb35bee6cd7afd556805f7b96ead0a9a7e45d785d8e4f3b719e2037c",
    "g36_connected_group_mode_validation.mos": "9a50c2a3b7378668c7d41952f8d61afd9be91607d3e2af00d08870cfdd638280",
    "g36_connected_group_mode_openmodelica_raw.csv": "398b3e933729bb572e6f0ec6bf244251073deb08619e9fb6b6602247182bb056",
}
MODE_INPUTS = {
    "uGroOcc": "u1Occ", "nexOcc": "tNexOcc", "yCooTim": "maxCooDowTim",
    "yHigOccCoo": "u1HigOccCoo", "yWarTim": "maxWarUpTim",
    "yOccHeaHig": "u1OccHeaHig", "yOpeWin": "uOpeWin",
    "yColZon": "totColZon", "ySetBac": "u1SetBac",
    "yEndSetBac": "u1EndSetBac", "TZonMin": "TZonMin",
    "yHotZon": "totHotZon", "ySetUp": "u1SetUp",
    "yEndSetUp": "u1EndSetUp",
}
STEP = 36
STOP = 3600


def graph() -> dict:
    aggregate = group_status(2, 3)
    mode = operation_mode(n_zones=3)
    aggregate["name"] = "group_status"
    mode["name"] = "operation_mode"
    connections = [
        {"from": port["name"], "to": f"group_status.{port['name']}"}
        for port in aggregate["inputs"]
    ]
    connections.extend(
        {"from": f"group_status.{source}", "to": f"operation_mode.{target}"}
        for source, target in MODE_INPUTS.items()
    )
    connections.append({"from": "operation_mode.yOpeMod", "to": "yOpeMod"})
    return {
        "name": "G36ConnectedGroupModeValidation", "block_type": "GroupMode",
        "category": "composite",
        "description": "Pinned OBC aggregate status wired into pinned OBC operating mode.",
        "inputs": aggregate["inputs"],
        "outputs": [{"name": "yOpeMod", "type": "Integer"}],
        "blocks": [aggregate, mode], "connections": connections,
    }


def trend(aggregate: dict) -> str:
    rows: dict[int, dict[str, str]] = {}
    with (VERIFY / "g36_connected_group_mode_openmodelica_raw.csv").open(
        newline="", encoding="utf-8"
    ) as handle:
        for row in csv.DictReader(handle):
            time = float(row["time"])
            tick = round(time / STEP)
            if abs(time - tick * STEP) <= 1e-3 and 0 <= tick * STEP <= STOP:
                rows[tick * STEP] = row  # last settled event at each sample
    if sorted(rows) != list(range(0, STOP + STEP, STEP)):
        raise ValueError("connected GroupStatus/OperationMode trace does not cover 101 samples")
    with io.StringIO(newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(("time_s", *(port["name"] for port in aggregate["inputs"]), "y"))
        for time in range(0, STOP + STEP, STEP):
            row = rows[time]
            values = []
            for port in aggregate["inputs"]:
                base, zone = port["name"].rsplit("_", 1)
                values.append(row[f"zonGroSta.{base}[{(1, 3)[int(zone) - 1]}]"])
            writer.writerow((time, *values, row["opeModSel.yOpeMod"]))
        return handle.getvalue()


def main() -> None:
    if sys.argv[1:] not in ([], ["--check"]):
        raise SystemExit("usage: generate_connected_group_mode_reference.py [--check]")
    for name, expected in PINNED.items():
        if hashlib.sha256((VERIFY / name).read_bytes()).hexdigest() != expected:
            raise ValueError(f"pinned connected group mode reference changed: {name}")
    building = json.loads((ROOT / "web/public/examples/four_pipe_g36.json").read_text())
    for group_name in ("zone_group_1", "zone_group_2"):
        group = next(block for block in building["blocks"] if block["name"] == group_name)
        wires = {(edge["from"], edge["to"]) for edge in group["connections"]}
        missing = [
            (f"obc_group_status.{source}", f"obc_operation_mode.{target}")
            for source, target in MODE_INPUTS.items()
            if (f"obc_group_status.{source}", f"obc_operation_mode.{target}") not in wires
        ]
        if missing:
            raise ValueError(f"{group_name} is missing OBC mode connections: {missing}")
    model = graph()
    files = {
        VERIFY / "g36_connected_group_mode_validation.json":
            json.dumps(model, indent=2) + "\n",
        VERIFY / "g36_connected_group_mode_yOpeMod.csv":
            trend(model["blocks"][0]),
    }
    for target, content in files.items():
        if sys.argv[1:] == ["--check"]:
            if target.read_text(encoding="utf-8") != content:
                raise ValueError(f"connected group mode fixture drift: {target}")
        else:
            target.write_text(content, encoding="utf-8", newline="")


if __name__ == "__main__":
    main()
