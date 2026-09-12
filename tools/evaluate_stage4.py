#!/usr/bin/env python3
"""Evaluate the predeclared stage-4 autonomous-behavior gates."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
from typing import Any

from analyze_behavior_trace import analyze


def load(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text())


def finite(value: Any) -> bool:
    if isinstance(value, float):
        return math.isfinite(value)
    if isinstance(value, dict):
        return all(finite(item) for item in value.values())
    if isinstance(value, list):
        return all(finite(item) for item in value)
    return True


def evaluate_run(payload: dict[str, Any]) -> dict[str, Any]:
    diagnosis = analyze(payload)
    samples = payload["samples"]
    flight_modes = diagnosis["modes"]["flight_mode"]
    total = sum(item["seconds"] for item in flight_modes.values())
    maximum_residency = max(item["seconds"] for item in flight_modes.values()) / total
    bounds = payload["room_bounds_mm"]
    inside = all(
        bounds[0][axis] - 1e-6 <= sample["root_position"][axis] <= bounds[1][axis] + 1e-6
        for sample in samples
        for axis in range(3)
    )
    realtime = payload["summary"]["duration_seconds"] / payload["summary"]["elapsed_seconds"]
    gates = {
        "duration_300s": payload["summary"]["duration_seconds"] >= 300.0,
        "flight_present": flight_modes.get("CRUISE", {}).get("seconds", 0.0) > 0.0,
        "supported_rest_at_least_3s": diagnosis["motion"]["supported_rest_seconds"] >= 3.0,
        "supported_crawling_present": diagnosis["motion"]["supported_crawling_seconds"] >= 0.5,
        "maximum_flight_bout_at_most_90s": flight_modes.get("CRUISE", {}).get(
            "longest_seconds", 0.0
        ) <= 90.0,
        "maximum_mode_residency_at_most_85pct": maximum_residency <= 0.85,
        "same_sign_turn_at_most_12s": diagnosis["motion"]["maximum_same_sign_turn_seconds"]
        <= 12.0,
        "no_three_consecutive_20s_circle_windows": diagnosis["repetition"][
            "maximum_consecutive_twenty_second_circle_windows"
        ]
        < 3,
        "occupancy_improved_or_at_least_20pct": diagnosis["motion"]["occupancy_coverage"]
        > 0.11212121212121212
        or diagnosis["motion"]["occupancy_coverage"] >= 0.20,
        "inside_room": inside,
        "all_values_finite": finite(payload),
        "realtime_at_least_0p95": realtime >= 0.95,
    }
    return {
        "seed": payload["initial_state"]["behavior_seed"],
        "realtime_factor": realtime,
        "feeding_seconds": payload["summary"]["feeding_seconds"],
        "minimum_hunger": min(sample["hunger"] for sample in samples),
        "maximum_fatigue": max(sample["fatigue"] for sample in samples),
        "maximum_mode_residency": maximum_residency,
        "diagnosis": diagnosis,
        "gates": gates,
        "passed": all(gates.values()),
    }


def fixture_summary(
    sugar: dict[str, Any], flower: dict[str, Any], disconnected: dict[str, Any]
) -> dict[str, Any]:
    sugar_samples = sugar["samples"]
    flower_samples = flower["samples"]
    disconnected_samples = disconnected["samples"]
    gates = {
        "sugar_neural_feeding": sugar["summary"]["feeding_seconds"] > 0.0
        and min(item["hunger"] for item in sugar_samples) < sugar_samples[0]["hunger"]
        and max(item["mn9_rate_hz"] for item in sugar_samples) > 0.0
        and max(item["feeding_extension"] for item in sugar_samples) > 0.1,
        "sugar_departure": sugar_samples[-1]["nearest_resource_distance_mm"]
        > sugar_samples[0]["nearest_resource_distance_mm"] + 20.0,
        "flower_sensory_approach": any(item["foraging_mode"] == "APPROACH" for item in flower_samples)
        and any(item["foraging_mode"] == "DESCEND" for item in flower_samples),
        "flower_neural_feeding": flower["summary"]["feeding_seconds"] > 0.0
        and 1 in {item["tasted_resource"] for item in flower_samples}
        and min(item["hunger"] for item in flower_samples) < flower_samples[0]["hunger"],
        "flower_departure": flower_samples[-1]["nearest_resource_distance_mm"]
        > min(item["nearest_resource_distance_mm"] for item in flower_samples) + 20.0,
        "disconnect_blocks_feeding": disconnected["summary"]["feeding_seconds"] == 0.0
        and min(item["hunger"] for item in disconnected_samples)
        >= disconnected_samples[0]["hunger"],
        "disconnect_blocks_active_exploration": max(
            abs(item["exploration_steering"]) for item in disconnected_samples
        )
        == 0.0,
    }
    return {"gates": gates, "passed": all(gates.values())}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--trace", action="append", type=Path, required=True)
    parser.add_argument("--sugar", type=Path, required=True)
    parser.add_argument("--flower", type=Path, required=True)
    parser.add_argument("--disconnected", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    runs = [evaluate_run(load(path)) for path in args.trace]
    fixtures = fixture_summary(load(args.sugar), load(args.flower), load(args.disconnected))
    feeding_runs = sum(run["feeding_seconds"] > 0.0 for run in runs)
    aggregate = {
        "three_runs": len(runs) == 3,
        "at_least_two_feeding_runs": feeding_runs >= 2,
        "all_run_gates": all(run["passed"] for run in runs),
        "all_fixture_gates": fixtures["passed"],
    }
    report = {
        "schema": "flybrain.stage4-gates-v1",
        "runs": runs,
        "fixtures": fixtures,
        "aggregate_gates": aggregate,
        "passed": all(aggregate.values()),
    }
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    if not report["passed"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
