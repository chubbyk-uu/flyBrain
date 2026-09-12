"""Sequential same-binary A/B tests; no model files or environment installs changed."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess


VARIANTS = {
    "baseline": {"FLYBRAIN_MUJOCO_MIDPHASE": "1", "FLYBRAIN_MUJOCO_BVACTIVE": "1"},
    "midphase-off": {"FLYBRAIN_MUJOCO_MIDPHASE": "0", "FLYBRAIN_MUJOCO_BVACTIVE": "1"},
    "bvactive-off": {"FLYBRAIN_MUJOCO_MIDPHASE": "1", "FLYBRAIN_MUJOCO_BVACTIVE": "0"},
    "both-off": {"FLYBRAIN_MUJOCO_MIDPHASE": "0", "FLYBRAIN_MUJOCO_BVACTIVE": "0"},
    "dense": {"FLYBRAIN_MUJOCO_MIDPHASE": "1", "FLYBRAIN_MUJOCO_BVACTIVE": "0",
              "FLYBRAIN_MUJOCO_JACOBIAN": "dense"},
}


def canonical(value):
    """Exclude timings/diagnostic switches, retain all neural and physical outputs."""
    if isinstance(value, dict):
        return {
            k: canonical(v) for k, v in value.items()
            if not (k.endswith("wall_seconds") or k in {
                "brain_encoding_seconds", "brain_engine_seconds", "elapsed_seconds",
                "non_brain_non_physics_seconds", "physics_profile",
            })
        }
    if isinstance(value, list):
        return [canonical(v) for v in value]
    return value


def first_difference(a, b, path="$"):
    if type(a) is not type(b):
        return {"path": path, "baseline": a, "candidate": b}
    if isinstance(a, dict):
        if a.keys() != b.keys():
            return {"path": path, "baseline_keys": list(a), "candidate_keys": list(b)}
        # Report the earliest sampled divergence before aggregate totals.
        for key in sorted(a, key=lambda k: (k != "samples", k)):
            diff = first_difference(a[key], b[key], f"{path}.{key}")
            if diff:
                return diff
    elif isinstance(a, list):
        if len(a) != len(b):
            return {"path": path, "baseline_length": len(a), "candidate_length": len(b)}
        for i, (x, y) in enumerate(zip(a, b)):
            diff = first_difference(x, y, f"{path}[{i}]")
            if diff:
                return diff
    elif a != b:
        return {"path": path, "baseline": a, "candidate": b}
    return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/release/flybrain-world"))
    parser.add_argument("--duration-seconds", type=float, default=20)
    parser.add_argument("--trials", type=int, default=3)
    parser.add_argument("--variants", nargs="+", choices=VARIANTS, default=list(VARIANTS))
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    if args.trials < 1 or args.duration_seconds <= 0:
        parser.error("trials and duration must be positive")
    args.output_dir.mkdir(parents=True, exist_ok=False)
    binary = args.binary.resolve()
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    baseline = None
    rows = []
    env = os.environ.copy()
    for name in ("LD_PRELOAD", "FLYBRAIN_PROFILE_COLLISION", "FLYBRAIN_MUJOCO_ENERGY"):
        env.pop(name, None)
    env["FLYBRAIN_PROFILE_PHYSICS"] = "1"
    env["FLYBRAIN_MUJOCO_JACOBIAN"] = "auto"
    for trial in range(1, args.trials + 1):
        order = args.variants if trial % 2 else list(reversed(args.variants))
        for variant in order:
            output = args.output_dir / f"{variant}-{trial}.json"
            command = [str(binary), "cns-check", "--duration-seconds",
                       str(args.duration_seconds), "--output", str(output)]
            with output.with_suffix(".log").open("x") as log:
                subprocess.run(command, env=env | VARIANTS[variant], stdout=log,
                               stderr=subprocess.STDOUT, check=True)
            report = json.loads(output.read_text())
            if report["runtime_sha256"] != digest:
                raise RuntimeError("binary changed during experiment")
            if baseline is None:
                baseline = canonical(report)
            summary = report["summary"]
            row = {
                "variant": variant, "trial": trial, "file": str(output),
                "elapsed_seconds": summary["elapsed_seconds"],
                "physics_seconds": summary["physics_wall_seconds"],
                "mujoco_seconds": summary["physics_profile"]["mujoco_step_wall_seconds"],
                "brain_seconds": summary["brain_wall_seconds"],
                "realtime_factor": args.duration_seconds / summary["elapsed_seconds"],
                "first_difference": first_difference(baseline, canonical(report)),
                "sha256": hashlib.sha256(output.read_bytes()).hexdigest(),
            }
            rows.append(row)
            print(json.dumps(row), flush=True)
    medians = {
        variant: {
            key: statistics.median(r[key] for r in rows if r["variant"] == variant)
            for key in ("elapsed_seconds", "physics_seconds", "mujoco_seconds", "brain_seconds", "realtime_factor")
        } for variant in args.variants
    }
    result = {"runtime_sha256": digest, "duration_seconds": args.duration_seconds,
              "reference": rows[0]["file"], "rows": rows, "medians": medians}
    with (args.output_dir / "summary.json").open("x") as stream:
        json.dump(result, stream, indent=2)
        stream.write("\n")
    print(json.dumps(medians, indent=2), flush=True)


if __name__ == "__main__":
    main()
