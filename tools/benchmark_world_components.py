"""Headless/viewer component ablations with the original body."""

import argparse
import hashlib
import json
import os
import statistics
import subprocess
from pathlib import Path

CASES = {
    "headless": {},
    "no-brain": {},
    "no-brain-no-retina-no-shadows": {
        "FLYBRAIN_BENCH_NO_RETINA": "1",
        "FLYBRAIN_VIEWER_SHADOWS": "0",
    },
    "full-viewer": {},
    "no-retina": {"FLYBRAIN_BENCH_NO_RETINA": "1"},
    "no-telemetry": {"FLYBRAIN_BENCH_NO_TELEMETRY": "1"},
    "no-shadows": {"FLYBRAIN_VIEWER_SHADOWS": "0"},
    "no-retina-no-shadows": {
        "FLYBRAIN_BENCH_NO_RETINA": "1",
        "FLYBRAIN_VIEWER_SHADOWS": "0",
    },
    "no-hud": {"FLYBRAIN_BENCH_NO_HUD": "1"},
    "no-main-details": {"FLYBRAIN_BENCH_NO_MAIN_DETAILS": "1"},
    "no-retina-no-main-details": {
        "FLYBRAIN_BENCH_NO_RETINA": "1",
        "FLYBRAIN_BENCH_NO_MAIN_DETAILS": "1",
    },
    "no-retina-no-shadows-no-hud": {
        "FLYBRAIN_BENCH_NO_RETINA": "1",
        "FLYBRAIN_VIEWER_SHADOWS": "0",
        "FLYBRAIN_BENCH_NO_HUD": "1",
    },
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--seconds", type=float, default=10)
    parser.add_argument("--trials", type=int, default=2)
    parser.add_argument("--trace", action="store_true")
    parser.add_argument("--cuda-probe", type=Path)
    parser.add_argument("--execution", choices=("graph", "direct"), default="graph")
    parser.add_argument("--physics-dt-ms", type=float)
    parser.add_argument("--width", type=int, default=1280)
    parser.add_argument("--height", type=int, default=800)
    parser.add_argument("--fps", type=int, default=60)
    parser.add_argument("--camera", default="chase")
    parser.add_argument("--shadow-map-size", type=int, default=1024)
    parser.add_argument("--msaa-samples", type=int, choices=(0, 2, 4, 8), default=2)
    parser.add_argument("--cases", choices=CASES, nargs="+", default=list(CASES))
    args = parser.parse_args()
    if (
        args.seconds <= 0
        or args.trials < 1
        or args.width <= 0
        or args.height <= 0
        or args.fps <= 0
        or args.shadow_map_size <= 0
        or (args.physics_dt_ms is not None and args.physics_dt_ms <= 0)
    ):
        parser.error("duration and trials must be positive")
    if args.cuda_probe and args.execution != "graph":
        parser.error("the CUDA event probe requires graph execution")
    args.output_dir.mkdir(parents=True, exist_ok=False)
    binary = Path("target/release/flybrain-world").resolve()
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    env = {
        k: v
        for k, v in os.environ.items()
        if not k.startswith("FLYBRAIN_") and k not in ("LD_PRELOAD", "LD_LIBRARY_PATH")
    }
    env.update(
        FLYBRAIN_CUDA_EXECUTION=args.execution,
        FLYBRAIN_PROFILE_PHYSICS="1",
        FLYBRAIN_PROFILE_VIEWER="1",
    )
    if args.cuda_probe:
        env["LD_PRELOAD"] = str(args.cuda_probe.resolve(strict=True))
    rows = []
    for trial in range(1, args.trials + 1):
        cases = args.cases if trial % 2 else args.cases[::-1]
        for case in cases:
            stem = args.output_dir / f"{case}-{trial}"
            command = [str(binary)]
            if case == "headless":
                command += [
                    "cns-check",
                    "--duration-seconds",
                    str(args.seconds),
                    "--output",
                    str(stem.with_suffix(".json")),
                ]
            else:
                command += [
                    "view",
                    "--max-seconds",
                    str(args.seconds),
                    "--width",
                    str(args.width),
                    "--height",
                    str(args.height),
                    "--fps",
                    str(args.fps),
                    "--camera",
                    args.camera,
                    "--shadow-map-size",
                    str(args.shadow_map_size),
                    "--msaa-samples",
                    str(args.msaa_samples),
                ]
                if case.startswith("no-brain"):
                    command += ["--no-brain"]
            if args.physics_dt_ms is not None:
                command += ["--physics-dt-ms", str(args.physics_dt_ms)]
            if args.trace:
                command = [
                    "nsys",
                    "profile",
                    "--trace=cuda",
                    "--sample=none",
                    "--cpuctxsw=none",
                    "--cuda-graph-trace=node",
                    f"--output={stem}",
                ] + command
            with stem.with_suffix(".log").open("x") as log:
                subprocess.run(
                    command, env=env | CASES[case], stdout=log, stderr=subprocess.STDOUT, check=True
                )
            if hashlib.sha256(binary.read_bytes()).hexdigest() != digest:
                raise RuntimeError("binary changed during benchmark")
            row = {"case": case, "trial": trial, "traced": args.trace}
            for line in stem.with_suffix(".log").read_text().splitlines():
                if line.startswith("CUDABENCH "):
                    row["cuda"] = json.loads(line[len("CUDABENCH ") :])
            if args.cuda_probe and "cuda" not in row:
                raise RuntimeError("CUDA probe did not load")
            if args.cuda_probe and case != "no-brain":
                cuda = row["cuda"]
                if not cuda["graphs"] or cuda["graphs"] != cuda["measured_graphs"]:
                    raise RuntimeError("incomplete CUDA graph event measurements")
            if case == "headless":
                report = json.loads(stem.with_suffix(".json").read_text())
                row["worker"] = report["summary"]
            else:
                render = []
                for line in stem.with_suffix(".log").read_text().splitlines():
                    for marker, key in [("WORKERBENCH ", "worker"), ("VIEWBENCH ", "viewer")]:
                        if line.startswith(marker):
                            row[key] = json.loads(line[len(marker) :])
                    if line.startswith("RENDERBENCH "):
                        render.append(json.loads(line[len("RENDERBENCH ") :]))
                if not render or "worker" not in row or "viewer" not in row:
                    raise RuntimeError("missing viewer profiling output")
                row["render_totals"] = {k: sum(r[k] for r in render) for k in render[0]}
                row["render_mean_ms"] = {
                    k: statistics.mean(r[k] for r in render) * 1000
                    for k in render[0]
                    if k.endswith("seconds")
                }
                row["fps"] = row["viewer"]["frames"] / row["viewer"]["wall_seconds"]
            rows.append(row)
            print(json.dumps(row), flush=True)
            (args.output_dir / "summary.json").write_text(
                json.dumps(
                    {
                        "binary_sha256": digest,
                        "duration_seconds": args.seconds,
                        "physics_dt_ms": args.physics_dt_ms,
                        "resolution": [args.width, args.height],
                        "requested_fps": args.fps,
                        "camera": args.camera,
                        "shadow_map_size": args.shadow_map_size,
                        "msaa_samples": args.msaa_samples,
                        "command": command,
                        "environment": {
                            **{k: v for k, v in env.items() if k.startswith("FLYBRAIN_")},
                            **CASES[case],
                        },
                        "rows": rows,
                    },
                    indent=2,
                )
                + "\n"
            )


if __name__ == "__main__":
    main()
