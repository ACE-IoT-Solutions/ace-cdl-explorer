#!/usr/bin/env python3
"""Replay CDL input trends through Rust and compare outputs with LBNL's funnel."""

import csv
import hashlib
import json
import math
import shutil
import subprocess
import sys
from pathlib import Path

import pyfunnel


ROOT = Path(__file__).resolve().parents[1]
SUITE = ROOT / "verification" / "suite.json"
RESULTS = ROOT / "verification" / "results"
PINNED_REFERENCE_FILES = {
    "g36_damper_valves_sequence.mo": "b1c4611d2a10cfcd1d73ecb3b9370485f0c1ab37f1296600a345387cf3b62eac",
    "g36_damper_valves_modelica.mo": "00279513ee69285442f6dd98862d21ec4dcd24214c82494060a2e56f2e4d2c23",
    "g36_damper_valves_dymola.txt": "aaa2ef3c2cfb098692f92b0715cd795d4a8289a44dc60b893c23685d403c2385",
    "g36_common_vout_flow_modelica.mo": "1329c6fe2945740bfc6cc45ccd1ca557e23a691da99cd66cc556f5be0a7d65fd",
    "g36_common_vout_flow_validation.mo": "8b948980ce73d2f167ed6d635e52c0125758789fc753f1a148b82084100ee6c2",
    "g36_common_vout_flow_dymola.txt": "438d37e80c17703cae9d2f150507f4b4cde4b134d320368788691b35134174c7",
    "g36_common_loop_disable_validation.mo": "577dc865d1c1d4ea43421ddad6c768c9d8adc3965b84ebbc3bf2871da0b6b7b2",
    "g36_common_loop_disable_dymola.txt": "2b052523096eb6f097515fbde243cfb2c16244fc5e60958753932cc5bbc9a8ae",
    "g36_ahu_freeze_sequence_modelica.mo": "f4e5c6e37235e4c04be48c3b77e156d33b426f53bef2c92148217dc7877133d1",
    "g36_ahu_freeze_validation_modelica.mo": "53e7ee9b6e29f1ee93834342b3e716e240682421412824f420967d150ddaa77f",
    "g36_ahu_freeze_dymola.txt": "845a48240e7388f5175a7f287bcbfa08ddea4256855ea229cd0b0d0a8e3ca949",
}


def read_trace(path: Path) -> tuple[list[float], list[float]]:
    with path.open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    if not rows:
        raise ValueError(f"{path}: no trace rows")
    time = [float(row["time_s"]) for row in rows]
    values = [float(row["y"]) for row in rows]
    if not all(math.isfinite(value) for value in time + values):
        raise ValueError(f"{path}: nonfinite trace value")
    if any(later <= earlier for earlier, later in zip(time, time[1:])):
        raise ValueError(f"{path}: time must strictly increase")
    return time, values


def main() -> int:
    for name, expected in PINNED_REFERENCE_FILES.items():
        actual = hashlib.sha256((SUITE.parent / name).read_bytes()).hexdigest()
        if actual != expected:
            raise ValueError(f"pinned OBC reference changed: {name}")
    suite = json.loads(SUITE.read_text(encoding="utf-8"))
    subprocess.run([sys.executable, str(ROOT / "scripts/generate_optimal_start_reference.py"),
                    "--check"], check=True, cwd=ROOT)
    subprocess.run([sys.executable, str(ROOT / "scripts/generate_optimal_start_modelica_reference.py"),
                    "--check"], check=True, cwd=ROOT)
    subprocess.run([sys.executable, str(ROOT / "scripts/generate_zone_status_reference.py"),
                    "--check"], check=True, cwd=ROOT)
    subprocess.run([sys.executable, str(ROOT / "scripts/generate_zone_status_modelica_reference.py"),
                    "--check"], check=True, cwd=ROOT)
    subprocess.run([sys.executable, str(ROOT / "scripts/obc_operation_mode.py"),
                    "--check"], check=True, cwd=ROOT)
    subprocess.run([sys.executable, str(ROOT / "scripts/generate_operation_mode_reference.py"),
                    "--check"], check=True, cwd=ROOT)
    subprocess.run([sys.executable, str(ROOT / "scripts/obc_group_status.py"),
                    "--check"], check=True, cwd=ROOT)
    subprocess.run([sys.executable, str(ROOT / "scripts/generate_group_status_reference.py"),
                    "--check"], check=True, cwd=ROOT)
    subprocess.run([sys.executable, str(ROOT / "scripts/generate_group_status_modelica_reference.py"),
                    "--check"], check=True, cwd=ROOT)
    subprocess.run([sys.executable, str(ROOT / "scripts/generate_connected_group_mode_reference.py"),
                    "--check"], check=True, cwd=ROOT)
    subprocess.run([sys.executable, str(ROOT / "scripts/generate_g36_mode_priority_reference.py"),
                    "--check"], check=True, cwd=ROOT)
    subprocess.run([sys.executable, str(ROOT / "scripts/generate_pressure_requests_modelica_reference.py"),
                    "--check"], check=True, cwd=ROOT)
    subprocess.run([sys.executable, str(ROOT / "scripts/obc_optimal_start_calculation.py"),
                    "--check"], check=True, cwd=ROOT)
    subprocess.run([sys.executable, str(ROOT / "scripts/integrate_obc_optimal_start.py"),
                    "--check"], check=True, cwd=ROOT)
    subprocess.run([sys.executable, str(ROOT / "scripts/generate_reheat_alarm_reference.py"),
                    "--check"], check=True, cwd=ROOT)
    subprocess.run([sys.executable, str(ROOT / "scripts/generate_leaking_valve_modelica_reference.py"),
                    "--check"], check=True, cwd=ROOT)
    subprocess.run([sys.executable, str(ROOT / "scripts/generate_leaking_damper_modelica_reference.py"),
                    "--check"], check=True, cwd=ROOT)
    subprocess.run([sys.executable, str(ROOT / "scripts/generate_low_flow_importance_modelica_reference.py"),
                    "--check"], check=True, cwd=ROOT)
    subprocess.run([sys.executable, str(ROOT / "scripts/generate_supply_trim_respond_modelica_reference.py"),
                    "--check"], check=True, cwd=ROOT)
    subprocess.run([sys.executable, str(ROOT / "scripts/generate_time_suppression_repeated_modelica_reference.py"),
                    "--check"], check=True, cwd=ROOT)
    if RESULTS.exists():
        shutil.rmtree(RESULTS)
    actual_dir = RESULTS / "actual"
    actual_dir.mkdir(parents=True)
    subprocess.run(
        ["cargo", "run", "--quiet", "--manifest-path", str(ROOT / "runtime/Cargo.toml"),
         "--bin", "obc_verify", "--", str(SUITE), str(actual_dir)],
        check=True, cwd=ROOT,
    )
    report = []
    for case in suite["cases"]:
        reference_time, reference_y = read_trace(SUITE.parent / case["trend"])
        time_tolerance = float(case["time_tolerance_s"])
        value_tolerance = float(case["value_tolerance"])
        if time_tolerance < 0 or value_tolerance < 0:
            raise ValueError(f"{case['id']}: negative tolerance")
        for engine, suffix in (("dynamic", ""), ("compiled", "_compiled")):
            actual_time, actual_y = read_trace(actual_dir / f"{case['id']}{suffix}.csv")
            if len(reference_time) != len(actual_time):
                raise ValueError(f"{case['id']} {engine}: reference and runtime sample counts differ")
            if case["id"] == "g36_time_suppression_repeated_after":
                differing = [time for time, expected, actual in zip(reference_time, reference_y, actual_y)
                             if expected != actual]
                if differing != [900.0, 2940.0]:
                    raise ValueError(f"{case['id']} {engine}: unexpected event-boundary differences {differing}")
            output = RESULTS / f"{case['id']}{suffix}"
            pyfunnel.compareAndReport(
                xReference=reference_time, yReference=reference_y,
                xTest=actual_time, yTest=actual_y,
                outputDirectory=str(output), atolx=time_tolerance, atoly=value_tolerance,
            )
            with (output / "errors.csv").open(newline="", encoding="utf-8") as handle:
                errors = [abs(float(row["y"])) for row in csv.DictReader(handle)]
            if len(errors) != len(actual_time) or not all(math.isfinite(error) for error in errors):
                raise ValueError(f"{case['id']} {engine}: invalid funnel result")
            maximum = max(errors, default=0.0)
            passed = maximum <= 1e-10
            report.append({"case": case["id"], "engine": engine, "samples": len(errors),
                           "max_excess": maximum, "passed": passed})
            print(f"{'PASS' if passed else 'FAIL'} {case['id']} {engine}: {len(errors)} samples, max excess {maximum:.6g}")
    (RESULTS / "summary.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return 0 if all(item["passed"] for item in report) else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"OBC verification error: {error}", file=sys.stderr)
        sys.exit(2)
