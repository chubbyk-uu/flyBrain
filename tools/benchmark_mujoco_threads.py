"""Headless same-binary thread-pool/library screening; never changes defaults."""

import argparse
import hashlib
import json
import os
import statistics
import subprocess
from pathlib import Path

from benchmark_mujoco_runtime import canonical, first_difference


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--probe", type=Path, required=True)
    parser.add_argument("--library-dir", type=Path, default=Path("work/mujoco/lib"))
    parser.add_argument("--workers", type=int, nargs="+", default=[0, 2, 4, 8])
    parser.add_argument("--trials", type=int, default=3)
    parser.add_argument("--seconds", type=float, default=10)
    parser.add_argument("--physics-dt-ms", type=float, default=0.2)
    args = parser.parse_args()
    if args.trials < 1 or args.seconds <= 0 or any(w < 0 or w > 8 for w in args.workers):
        parser.error("invalid trial count, duration or workers")
    args.output_dir.mkdir(parents=True, exist_ok=False)
    binary = Path("target/release/flybrain-world").resolve()
    probe = args.probe.resolve(strict=True)
    library_dir = args.library_dir.resolve(strict=True)
    library = (library_dir / "libmujoco.so.3.9.0").resolve(strict=True)
    metadata = {
        "binary_sha256": sha(binary),
        "probe_sha256": sha(probe),
        "library": str(library),
        "library_sha256": sha(library),
        "physics_dt_ms": args.physics_dt_ms,
        "seconds": args.seconds,
    }
    env = os.environ.copy()
    for key in list(env):
        if key.startswith("FLYBRAIN_") or key == "LD_PRELOAD":
            del env[key]
    env.update(
        LD_PRELOAD=str(probe),
        LD_LIBRARY_PATH=str(library_dir) + ":" + env.get("LD_LIBRARY_PATH", ""),
        FLYBRAIN_CUDA_EXECUTION="graph",
        FLYBRAIN_PROFILE_PHYSICS="1",
    )
    baseline = None
    references = {}
    rows = []
    for trial in range(1, args.trials + 1):
        for workers in args.workers if trial % 2 else args.workers[::-1]:
            path = args.output_dir / f"workers-{workers}-trial-{trial}.json"
            command = [
                str(binary),
                "cns-check",
                "--duration-seconds",
                str(args.seconds),
                "--physics-dt-ms",
                str(args.physics_dt_ms),
                "--output",
                str(path),
            ]
            with path.with_suffix(".log").open("x") as log:
                subprocess.run(
                    command,
                    env=env | {"FLYBRAIN_PROBE_WORKERS": str(workers)},
                    stdout=log,
                    stderr=subprocess.STDOUT,
                    check=True,
                )
            if f"THREADPROBE workers={workers} " not in path.with_suffix(".log").read_text():
                raise RuntimeError("probe did not load")
            report = json.loads(path.read_text())
            if report["runtime_sha256"] != metadata["binary_sha256"]:
                raise RuntimeError("binary changed")
            result = canonical(report)
            if baseline is None:
                baseline = result
            references.setdefault(workers, result)
            summary = report["summary"]
            row = {
                "workers": workers,
                "trial": trial,
                "sha256": sha(path),
                "elapsed": summary["elapsed_seconds"],
                "physics": summary["physics_wall_seconds"],
                "brain": summary["brain_wall_seconds"],
                "mj_step": summary["physics_profile"]["mujoco_step_wall_seconds"],
                "rtf": args.seconds / summary["elapsed_seconds"],
                "flight": summary["flight_seconds"],
                "feed": summary["feeding_seconds"],
                "first_difference": first_difference(baseline, result),
                "repeat_difference": first_difference(references[workers], result),
            }
            rows.append(row)
            print(json.dumps(row), flush=True)
    medians = {
        w: {
            k: statistics.median(r[k] for r in rows if r["workers"] == w)
            for k in ("elapsed", "physics", "brain", "mj_step", "rtf")
        }
        for w in args.workers
    }
    metadata.update(rows=rows, medians=medians)
    (args.output_dir / "summary.json").write_text(json.dumps(metadata, indent=2) + "\n")
    print(json.dumps(medians), flush=True)


if __name__ == "__main__":
    main()
