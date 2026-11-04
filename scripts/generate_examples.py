"""Generate a deterministic 2025 mixed-climate teaching scenario."""
import json
import math
from datetime import datetime, timedelta, timezone
from pathlib import Path
from control_graph import building, vav

ROOT = Path(__file__).resolve().parents[1] / "web" / "public" / "examples"
(ROOT / "four_pipe_building.json").write_text(json.dumps(building(), indent=2) + "\n")
(ROOT / "vav_composite_template.json").write_text(json.dumps(vav("VAVController"), indent=2) + "\n")

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
