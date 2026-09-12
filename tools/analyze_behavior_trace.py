#!/usr/bin/env python3
"""Analyze a flybrain.cns-world-check trace without changing simulation behavior."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
from typing import Any

WINDOW_SECONDS = 10.0
GRID_MM = 20.0
CIRCLE_MIN_PATH_MM = 50.0
CIRCLE_MIN_TURN_RAD = 1.5 * math.tau
CIRCLE_MAX_CLOSURE_RATIO = 0.35
GRID_LOOP_MIN_UNIQUE_CELLS = 6
SHUTTLE_MIN_CORRELATION = 0.85
SHUTTLE_MIN_LAG_SECONDS = 2.0
SHUTTLE_MAX_LAG_SECONDS = 10.0
TURN_DEADBAND = 0.05


def _distance(a: list[float], b: list[float]) -> float:
    return math.hypot(b[0] - a[0], b[1] - a[1])


def _wrap(angle: float) -> float:
    return (angle + math.pi) % math.tau - math.pi


def detect_repetition(samples: list[dict[str, Any]]) -> dict[str, Any]:
    if len(samples) < 3:
        return {"circle_windows": 0, "grid_loop_windows": 0, "shuttle_windows": 0}
    dt = _median_dt(samples)
    window = max(3, round(WINDOW_SECONDS / dt))
    circle = grid_loop = shuttle = 0
    for start in range(0, len(samples) - window + 1, window):
        chunk = samples[start : start + window]
        points = [sample["root_position"] for sample in chunk]
        segments = [_distance(a, b) for a, b in zip(points, points[1:])]
        path = sum(segments)
        headings = [
            math.atan2(b[1] - a[1], b[0] - a[0])
            for a, b, length in zip(points, points[1:], segments)
            if length > 1e-6
        ]
        turn = sum(_wrap(b - a) for a, b in zip(headings, headings[1:]))
        closure = _distance(points[0], points[-1]) / max(path, 1e-9)
        if (
            path >= CIRCLE_MIN_PATH_MM
            and abs(turn) >= CIRCLE_MIN_TURN_RAD
            and closure <= CIRCLE_MAX_CLOSURE_RATIO
        ):
            circle += 1
        cells = {(math.floor(p[0] / GRID_MM), math.floor(p[1] / GRID_MM)) for p in points}
        if len(cells) >= GRID_LOOP_MIN_UNIQUE_CELLS and closure <= CIRCLE_MAX_CLOSURE_RATIO:
            grid_loop += 1
        if _maximum_planar_autocorrelation(points, dt) >= SHUTTLE_MIN_CORRELATION:
            shuttle += 1
    return {
        "window_seconds": WINDOW_SECONDS,
        "windows": max(0, (len(samples) - window) // window + 1),
        "circle_windows": circle,
        "grid_loop_windows": grid_loop,
        "shuttle_windows": shuttle,
        "thresholds": {
            "circle_min_path_mm": CIRCLE_MIN_PATH_MM,
            "circle_min_abs_turn_rad": CIRCLE_MIN_TURN_RAD,
            "circle_max_closure_ratio": CIRCLE_MAX_CLOSURE_RATIO,
            "grid_mm": GRID_MM,
            "grid_loop_min_unique_cells": GRID_LOOP_MIN_UNIQUE_CELLS,
            "shuttle_min_correlation": SHUTTLE_MIN_CORRELATION,
            "shuttle_lag_seconds": [SHUTTLE_MIN_LAG_SECONDS, SHUTTLE_MAX_LAG_SECONDS],
        },
    }


def _maximum_planar_autocorrelation(points: list[list[float]], dt: float) -> float:
    centered = []
    means = [sum(p[axis] for p in points) / len(points) for axis in range(2)]
    for point in points:
        centered.append([point[0] - means[0], point[1] - means[1]])
    energy = sum(x * x + y * y for x, y in centered)
    if energy <= 1e-9:
        return 0.0
    first = max(1, round(SHUTTLE_MIN_LAG_SECONDS / dt))
    last = min(len(points) - 2, round(SHUTTLE_MAX_LAG_SECONDS / dt))
    values = []
    for lag in range(first, last + 1):
        numerator = sum(
            a[0] * b[0] + a[1] * b[1]
            for a, b in zip(centered[:-lag], centered[lag:])
        )
        denominator = math.sqrt(
            sum(x * x + y * y for x, y in centered[:-lag])
            * sum(x * x + y * y for x, y in centered[lag:])
        )
        if denominator > 0:
            values.append(numerator / denominator)
    return max(values, default=0.0)


def _median_dt(samples: list[dict[str, Any]]) -> float:
    values = sorted(
        b["time_seconds"] - a["time_seconds"] for a, b in zip(samples, samples[1:])
    )
    return values[len(values) // 2]


def _sample_durations(samples: list[dict[str, Any]]) -> list[float]:
    """Assign each sample the interval until its successor.

    The trace sampler intentionally catches up on the next 2 ms control boundary,
    so nominal 10 ms samples occasionally span 12 ms.  Using one median interval
    for every sample would under-count long bouts in a 300 s trace.
    """
    deltas = [
        b["time_seconds"] - a["time_seconds"]
        for a, b in zip(samples, samples[1:])
    ]
    return deltas + [_median_dt(samples)]


def _bout_stats(samples: list[dict[str, Any]], key: str) -> dict[str, Any]:
    result: dict[str, dict[str, float | int]] = {}
    current = None
    duration = 0.0
    for sample, sample_duration in zip(samples, _sample_durations(samples)):
        value = str(sample[key])
        if value != current:
            if current is not None:
                item = result.setdefault(current, {"seconds": 0.0, "bouts": 0, "longest_seconds": 0.0})
                item["seconds"] = float(item["seconds"]) + duration
                item["bouts"] = int(item["bouts"]) + 1
                item["longest_seconds"] = max(float(item["longest_seconds"]), duration)
            current, duration = value, 0.0
        duration += sample_duration
    if current is not None:
        item = result.setdefault(current, {"seconds": 0.0, "bouts": 0, "longest_seconds": 0.0})
        item["seconds"] = float(item["seconds"]) + duration
        item["bouts"] = int(item["bouts"]) + 1
        item["longest_seconds"] = max(float(item["longest_seconds"]), duration)
    return result


def _longest_boolean_bout(samples: list[dict[str, Any]], predicate) -> float:
    longest = current = 0.0
    for sample, duration in zip(samples, _sample_durations(samples)):
        current = current + duration if predicate(sample) else 0.0
        longest = max(longest, current)
    return longest


def analyze(payload: dict[str, Any]) -> dict[str, Any]:
    samples = payload["samples"]
    if len(samples) < 2:
        raise ValueError("trace needs at least two samples")
    timestamps = [sample["time_seconds"] for sample in samples]
    deltas = [b - a for a, b in zip(timestamps, timestamps[1:])]
    if any(delta <= 0 for delta in deltas):
        raise ValueError("trace contains duplicate or decreasing timestamps")
    positions = [sample["root_position"] for sample in samples]
    path = sum(_distance(a, b) for a, b in zip(positions, positions[1:]))
    cells = {(math.floor(p[0] / GRID_MM), math.floor(p[1] / GRID_MM)) for p in positions}
    sample_durations = _sample_durations(samples)
    turn_runs: list[float] = []
    last_sign = 0
    run = 0.0
    dt = _median_dt(samples)
    for sample, sample_duration in zip(samples, sample_durations):
        value = sample["flight_steering"]
        sign = 1 if value > TURN_DEADBAND else -1 if value < -TURN_DEADBAND else 0
        if sign == 0:
            if run:
                turn_runs.append(run)
            last_sign, run = 0, 0.0
        elif sign == last_sign:
            run += sample_duration
        else:
            if run:
                turn_runs.append(run)
            last_sign, run = sign, sample_duration
    if run:
        turn_runs.append(run)
    room_bounds = payload.get("room_bounds_mm")
    usable_cells = None
    coverage = None
    if room_bounds:
        usable_cells = math.ceil(
            (room_bounds[1][0] - room_bounds[0][0]) / GRID_MM
        ) * math.ceil((room_bounds[1][1] - room_bounds[0][1]) / GRID_MM)
        coverage = len(cells) / usable_cells
    return {
        "schema": "flybrain.behavior-diagnosis-v1",
        "source_schema": payload.get("schema"),
        "trace": {
            "samples": len(samples),
            "start_seconds": timestamps[0],
            "end_seconds": timestamps[-1],
            "median_dt_seconds": dt,
            "maximum_dt_seconds": max(deltas),
            "duplicate_or_decreasing_timestamps": 0,
        },
        "modes": {
            key: _bout_stats(samples, key)
            for key in ("flight_mode", "behavior_mode", "foraging_mode")
        },
        "motion": {
            "path_length_mm": path,
            "net_displacement_mm": _distance(positions[0], positions[-1]),
            "visited_20mm_cells": len(cells),
            "usable_20mm_cells": usable_cells,
            "occupancy_coverage": coverage,
            "maximum_same_sign_turn_seconds": max(turn_runs, default=0.0),
            "longest_unsupported_flight_seconds": _longest_boolean_bout(
                samples,
                lambda sample: sample["flight_mode"] != "GROUNDED"
                and sample["contact_count"] == 0,
            ),
        },
        "events": {
            "contact_bouts": _bout_count(samples, lambda sample: sample["contact_count"] > 0),
            "taste_bouts": _bout_count(samples, lambda sample: sample["taste_active"]),
            "feeding_bouts": _bout_count(samples, lambda sample: sample["feeding_extension"] > 0.1),
        },
        "repetition": detect_repetition(samples),
    }


def _bout_count(samples: list[dict[str, Any]], predicate) -> int:
    active = False
    count = 0
    for sample in samples:
        value = bool(predicate(sample))
        if value and not active:
            count += 1
        active = value
    return count


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("trace", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    payload = json.loads(args.trace.read_text())
    report = analyze(payload)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
