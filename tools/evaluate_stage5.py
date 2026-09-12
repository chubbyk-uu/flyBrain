#!/usr/bin/env python3
"""Evaluate the predeclared stage-5 autonomous grooming gates."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
from typing import Any

from analyze_behavior_trace import _median_dt
from evaluate_stage4 import evaluate_run, finite, load

PHASE4_MEAN_REALTIME = (1.0132995413777268 + 1.0192776209856855 + 1.1077409171823376) / 3


def bouts(samples: list[dict[str, Any]]) -> list[list[dict[str, Any]]]:
    result: list[list[dict[str, Any]]] = []
    current: list[dict[str, Any]] = []
    for sample in samples:
        if sample.get("grooming_active", False):
            current.append(sample)
        elif current:
            result.append(current)
            current = []
    if current:
        result.append(current)
    return result


def completed_drops(samples: list[dict[str, Any]]) -> list[float]:
    drops = []
    for previous, current in zip(samples, samples[1:]):
        if current["grooming_completed_bouts"] > previous["grooming_completed_bouts"]:
            drops.append((previous["dirt"] - current["dirt"]) / max(previous["dirt"], 1e-12))
    return drops


def fixture_gates(
    high: dict[str, Any],
    low: dict[str, Any],
    probe_disconnected: dict[str, Any],
    motor_disconnected: dict[str, Any],
) -> dict[str, Any]:
    high_samples = high["samples"]
    high_bouts = bouts(high_samples)
    dt = _median_dt(high_samples)
    start = min(sample["time_seconds"] for sample in high_samples if sample["grooming_active"])
    bout_seconds = [len(bout) * dt for bout in high_bouts]
    rubbing = [
        sample
        for sample in high_samples
        if sample["grooming_active"] and 0.15 <= sample["grooming_phase"] <= 0.52
    ]
    reaching = [
        sample
        for sample in high_samples
        if sample["grooming_active"] and 0.45 <= sample["grooming_phase"] <= 0.90
    ]
    drops = completed_drops(high_samples)
    final_completion_time = max(
        sample["time_seconds"]
        for previous, sample in zip(high_samples, high_samples[1:])
        if sample["grooming_completed_bouts"] > previous["grooming_completed_bouts"]
    )
    walk_after = any(
        sample["time_seconds"] > final_completion_time
        and sample["flight_mode"] == "GROUNDED"
        and sample["walking_activation"] > 0.05
        and sample["horizontal_speed_mm_s"] > 0.5
        for sample in high_samples
    )
    low_samples = low["samples"]
    probe_samples = probe_disconnected["samples"]
    motor_samples = motor_disconnected["samples"]
    gates = {
        "high_dirt_starts_within_30s": start < 30.0,
        "stable_support_precedes_start": min(
            sample["grooming_stable_support_seconds"]
            for sample in high_samples
            if sample["grooming_active"]
        )
        >= 0.5,
        "bout_duration_1_to_8s": bool(bout_seconds)
        and all(1.0 <= seconds <= 8.0 for seconds in bout_seconds if seconds >= 1.0),
        "four_support_legs": min(
            sample["grooming_support_leg_count"]
            for sample in high_samples
            if sample["grooming_active"]
        )
        >= 4
        and min(sample["contact_count"] for sample in high_samples if sample["grooming_active"])
        >= 4,
        "forelegs_rub": min(sample["front_tarsi_distance_mm"] for sample in rubbing) < 1.5,
        "foreleg_reaches_head_or_eye": min(
            sample["front_tarsus_head_eye_min_distance_mm"] for sample in reaching
        )
        < 1.5,
        "completed_cleaning_reduces_dirt_30pct": bool(drops) and min(drops) >= 0.30,
        "returns_to_walking": walk_after,
        "low_or_hungry_fixture_does_not_start": max(
            sample["grooming_completed_bouts"] for sample in low_samples
        )
        == 0
        and not any(sample["grooming_active"] for sample in low_samples),
        "probe_disconnect_blocks_action": max(
            sample["grooming_dn_rate_hz"] for sample in probe_samples
        )
        == 0.0
        and max(sample["grooming_completed_bouts"] for sample in probe_samples) == 0,
        "motor_disconnect_blocks_action": max(
            sample["grooming_completed_bouts"] for sample in motor_samples
        )
        == 0
        and not any(sample["grooming_active"] for sample in motor_samples),
        "connected_probe_and_joint_trajectory_present": max(
            sample["grooming_dn_rate_hz"] for sample in high_samples
        )
        > 0.0
        and any(
            abs(value) > 0.05
            for sample in high_samples
            if sample["grooming_active"]
            for side in sample["front_leg_joint_controls"]
            for value in side
        ),
        "fixture_values_finite": all(
            finite(payload)
            for payload in [high, low, probe_disconnected, motor_disconnected]
        ),
    }
    return {
        "gates": gates,
        "passed": all(gates.values()),
        "start_seconds": start,
        "bout_seconds": bout_seconds,
        "completed_dirt_drops": drops,
        "minimum_support_contacts": min(
            sample["contact_count"] for sample in high_samples if sample["grooming_active"]
        ),
        "minimum_foreleg_rub_distance_mm": min(
            sample["front_tarsi_distance_mm"] for sample in rubbing
        ),
        "minimum_head_eye_distance_mm": min(
            sample["front_tarsus_head_eye_min_distance_mm"] for sample in reaching
        ),
    }


def run_gates(payload: dict[str, Any]) -> dict[str, Any]:
    stage4 = evaluate_run(payload)
    samples = payload["samples"]
    run_bouts = bouts(samples)
    completed = max(sample["grooming_completed_bouts"] for sample in samples)
    realtime = payload["summary"]["duration_seconds"] / payload["summary"]["elapsed_seconds"]
    fallen_bout = 0.0
    current = 0.0
    dt = _median_dt(samples)
    for sample in samples:
        if sample["flight_mode"] == "GROUNDED" and sample["contact_count"] < 3:
            current += dt
            fallen_bout = max(fallen_bout, current)
        else:
            current = 0.0
    gates = {
        "duration_300s": stage4["gates"]["duration_300s"],
        "flight_present": stage4["gates"]["flight_present"],
        "supported_rest_present": stage4["gates"]["supported_rest_at_least_3s"],
        "supported_crawling_present": stage4["gates"]["supported_crawling_present"],
        "inside_room": stage4["gates"]["inside_room"],
        "all_values_finite": stage4["gates"]["all_values_finite"],
        "autonomous_grooming_completed": completed >= 1,
        "grooming_bouts_1_to_8s": bool(run_bouts)
        and all(1.0 <= len(bout) * dt <= 8.0 for bout in run_bouts),
        "no_permanent_fall": fallen_bout < 5.0,
        "no_takeoff_during_grooming": all(
            sample["flight_mode"] == "GROUNDED"
            for sample in samples
            if sample["grooming_active"]
        ),
        "realtime_at_least_95pct_phase4": realtime >= 0.95 * PHASE4_MEAN_REALTIME,
    }
    return {
        "seed": payload["initial_state"]["behavior_seed"],
        "realtime_factor": realtime,
        "completed_grooming_bouts": completed,
        "maximum_fallen_bout_seconds": fallen_bout,
        "gates": gates,
        "passed": all(gates.values()),
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--trace", action="append", type=Path, required=True)
    parser.add_argument("--high", type=Path, required=True)
    parser.add_argument("--low", type=Path, required=True)
    parser.add_argument("--probe-disconnected", type=Path, required=True)
    parser.add_argument("--motor-disconnected", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    runs = [run_gates(load(path)) for path in args.trace]
    fixtures = fixture_gates(
        load(args.high),
        load(args.low),
        load(args.probe_disconnected),
        load(args.motor_disconnected),
    )
    aggregate = {
        "three_runs": len(runs) == 3,
        "at_least_two_grooming_runs": sum(
            run["completed_grooming_bouts"] > 0 for run in runs
        )
        >= 2,
        "all_run_gates": all(run["passed"] for run in runs),
        "all_fixture_gates": fixtures["passed"],
    }
    result = {
        "schema": "flybrain.stage5-gates-v1",
        "phase4_mean_realtime": PHASE4_MEAN_REALTIME,
        "runs": runs,
        "fixtures": fixtures,
        "aggregate_gates": aggregate,
        "passed": all(aggregate.values()),
    }
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))
    if not result["passed"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
