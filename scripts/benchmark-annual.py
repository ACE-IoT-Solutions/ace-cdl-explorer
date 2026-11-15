#!/usr/bin/env python3
"""Compare full-year native snapshots and runtime against a frozen baseline binary."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("candidate", type=Path)
    args = parser.parse_args()
    timings = []
    snapshots = []
    with tempfile.TemporaryDirectory(prefix="cdl-annual-benchmark-") as directory:
        directory = Path(directory)
        traces = directory / "traces.json"
        traces.write_text("[]")
        for label, binary in [("baseline", args.baseline), ("candidate", args.candidate)]:
            output = directory / f"{label}.json"
            start = time.perf_counter()
            subprocess.run([str(binary.resolve()), "four_pipe_g36.json", str(output),
                            "900", str(traces), "35040"], check=True)
            timings.append(time.perf_counter() - start)
            snapshots.append(json.loads(output.read_text()))
            print(f"{label}: {timings[-1]:.3f}s for all 35,040 weather intervals", flush=True)
    assert snapshots[0] == snapshots[1], "annual rows/summary differ"
    ratio = timings[1] / timings[0]
    print(f"Exact snapshot parity; candidate/baseline runtime ratio {ratio:.3f}", flush=True)
    assert ratio <= 1.05, "candidate exceeds the five-percent native performance limit"


if __name__ == "__main__":
    main()
