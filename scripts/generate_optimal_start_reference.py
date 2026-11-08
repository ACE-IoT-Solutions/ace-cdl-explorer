#!/usr/bin/env python3
"""Regenerate the bounded zone-recovery trace from pinned OBC slope equations.

The source is Buildings/Controls/OBC/Utilities/BaseClasses/OptimalStartCalculation.mo
at d3d8e6f76514dfbc7d65574c6b5a777b686fe1b7. It divides captured
temperature error by max(recovery time, 120 s), includes the initial moving-mean
sample, retains the last three samples, and floors the mean at 1/3600 K/s.
"""

import csv
import io
import sys
from pathlib import Path


OUTPUT = Path(__file__).resolve().parents[1] / "verification/g36_zone_recovery_learner.csv"
STEP = 60
DAY = 86400
FALLBACK = 1 / 3600
EPISODES = [(3600, 3600), (DAY + 3600, 7200), (2 * DAY + 3600, 14400)]


def main():
    samples = [0.0]  # OBC TriggeredMovingMean takes its initial input sample.
    rate = FALLBACK
    with io.StringIO(newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(("time_s", "error", "active", "window_closed", "fallback_rate",
                         "midnight", "sampled_need", "y"))
        for time in range(0, 4 * DAY + STEP, STEP):
            episode = next(((start, duration) for start, duration in EPISODES
                            if start <= time <= start + duration), None)
            active = episode is not None
            error = 0.05 if episode and time == episode[0] + episode[1] else 3.0 if active else 0.0
            midnight = time > 0 and time % DAY == 0
            sampled_need = 0.0 if time == 4 * DAY else 3.0
            if midnight and sampled_need > 0:
                prior_start, prior_duration = EPISODES[time // DAY - 1]
                samples.append(3.0 / max(prior_duration, 120))
                rate = max(sum(samples[-3:]) / min(len(samples), 3), FALLBACK)
            writer.writerow((time, error, int(active), 1, repr(FALLBACK), int(midnight), sampled_need,
                             repr(rate)))
        expected = handle.getvalue()
    if sys.argv[1:] == ["--check"]:
        if OUTPUT.read_text(encoding="utf-8") != expected:
            raise SystemExit(f"{OUTPUT} is out of date; regenerate the OBC reference trend")
    elif not sys.argv[1:]:
        OUTPUT.write_text(expected, encoding="utf-8", newline="")
    else:
        raise SystemExit("usage: generate_optimal_start_reference.py [--check]")


if __name__ == "__main__":
    main()
