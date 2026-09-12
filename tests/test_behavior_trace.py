import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from tools.analyze_behavior_trace import (
    _bout_stats,
    _longest_boolean_bout,
    _maximum_consecutive,
    detect_repetition,
)


def _samples(points, dt=0.02):
    return [
        {"time_seconds": index * dt, "root_position": [x, y, 20.0]}
        for index, (x, y) in enumerate(points)
    ]


def test_circle_detector_accepts_closed_turning_and_rejects_straight_motion():
    count = 501
    circle = [
        (30.0 * math.cos(4 * math.tau * i / (count - 1)), 30.0 * math.sin(4 * math.tau * i / (count - 1)))
        for i in range(count)
    ]
    straight = [(i * 0.5, 0.0) for i in range(count)]
    assert detect_repetition(_samples(circle))["circle_windows"] == 1
    assert detect_repetition(_samples(straight))["circle_windows"] == 0


def test_grid_and_autocorrelation_detectors_find_periodic_shuttle():
    count = 501
    shuttle = [(70.0 * math.sin(math.tau * i / 100), 10.0 * math.sin(math.tau * i / 50)) for i in range(count)]
    result = detect_repetition(_samples(shuttle))
    assert result["grid_loop_windows"] == 1
    assert result["shuttle_windows"] == 1


def test_bout_durations_use_actual_irregular_sample_intervals():
    samples = [
        {"time_seconds": 0.0, "mode": "A", "active": True},
        {"time_seconds": 0.01, "mode": "A", "active": True},
        {"time_seconds": 0.022, "mode": "B", "active": False},
        {"time_seconds": 0.032, "mode": "B", "active": False},
    ]
    stats = _bout_stats(samples, "mode")
    assert math.isclose(stats["A"]["seconds"], 0.022)
    assert math.isclose(stats["B"]["seconds"], 0.02)
    assert math.isclose(_longest_boolean_bout(samples, lambda item: item["active"]), 0.022)


def test_consecutive_window_count_does_not_merge_separate_runs():
    assert _maximum_consecutive([]) == 0
    assert _maximum_consecutive([0, 1, 4, 5, 6, 9]) == 3
