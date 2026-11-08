"""Generate a deterministic 2025 mixed-climate teaching scenario."""
import json
import math
from datetime import datetime, timedelta, timezone
from pathlib import Path
from control_graph import building, vav

ROOT = Path(__file__).resolve().parents[1] / "web" / "public" / "examples"
(ROOT / "four_pipe_building.json").write_text(json.dumps(building(), indent=2) + "\n")
(ROOT / "vav_composite_template.json").write_text(json.dumps(vav("VAVController"), indent=2) + "\n")

# Preserve the teaching values from the original python-cdl VAV examples, but
# expose their controls as editable elementary CDL blocks. The old PI integral
# gain was applied once per 900-second step, so Ti = 900 * k / ki.
legacy_vav_tuning = {
    "east": (23.5, 21.0, 0.2, 0.95, 0.15, 0.6, 0.12, 0.35, 0.06),
    "north": (24.0, 21.0, 0.2, 0.9, 0.15, 0.55, 0.11, 0.35, 0.06),
    "south": (23.0, 21.0, 0.2, 1.0, 0.15, 0.6, 0.12, 0.35, 0.06),
    "west": (23.5, 21.0, 0.2, 0.95, 0.15, 0.6, 0.12, 0.35, 0.06),
    "corridor": (24.0, 21.0, 0.15, 0.6, 0.15, 0.5, 0.1, 0.3, 0.05),
}
for zone, values in legacy_vav_tuning.items():
    cool, heat, min_flow, max_flow, min_damper, cool_k, cool_ki, heat_k, heat_ki = values
    graph = vav(f"VAVController_zone_{zone}")
    children = {block["name"]: block for block in graph["blocks"]}

    def set_parameter(child, parameter, value):
        next(item for item in children[child]["parameters"] if item["name"] == parameter)["value"] = value

    for child, value in {
        "cool_setpoint": cool,
        "heat_setpoint": heat,
        "minimum_airflow": min_flow,
        "minimum_damper": min_damper,
        "flow_span": max_flow - min_flow,
    }.items():
        set_parameter(child, "k", value)
    set_parameter("cool_pi", "k", cool_k)
    set_parameter("cool_pi", "Ti", 900 * cool_k / cool_ki)
    set_parameter("cool_pi", "yMax", 1 - min_damper)
    set_parameter("heat_pi", "k", heat_k)
    set_parameter("heat_pi", "Ti", 900 * heat_k / heat_ki)
    set_parameter("damper_limit", "uMin", min_damper)
    graph["notes"] = f"Legacy {zone} VAV values, represented as editable CDL blocks."
    (ROOT / f"{zone}_controller.json").write_text(json.dumps(graph, indent=2) + "\n")

start = datetime(2025, 1, 1, tzinfo=timezone.utc)
weather = []
for i in range(365 * 96):
    t = start + timedelta(minutes=15 * i)
    day = i / 96
    hour = (i % 96) / 4
    annual = 11.0 + 13.0 * math.sin(2 * math.pi * (day - 105) / 365)
    diurnal = 5.0 * math.sin(2 * math.pi * (hour - 9) / 24)
    synoptic = 3.0 * math.sin(2 * math.pi * day / 9.7) + 1.5 * math.sin(2 * math.pi * day / 23.3)
    occupied = t.weekday() < 5 and 8 <= hour < 18
    sunlight = max(0.0, math.sin(math.pi * (hour - 6) / 12))
    season = max(0.25, 0.65 + 0.35 * math.sin(2 * math.pi * (day - 80) / 365))
    weather.append({
        "outdoor_c": round(annual + diurnal + synoptic, 2),
        "solar_kw": round(24.0 * sunlight * season, 2),
        "internal_kw": 16.0 if occupied else 1.2,
        "occupied": occupied,
    })
(ROOT / "weather_2025.json").write_text(json.dumps(weather, separators=(",", ":")) + "\n")
