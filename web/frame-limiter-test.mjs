import assert from "node:assert/strict";
import { FrameLimiter } from "./frame-limiter.js";

function sample(targetHz, displayHz, seconds) {
  const limiter = new FrameLimiter(targetHz);
  const times = [];
  for (let frame = 0; frame < displayHz * seconds; frame += 1) {
    const now = frame * 1000 / displayHz;
    if (limiter.due(now)) times.push(now);
  }
  const intervals = times.slice(1).map((time, index) => time - times[index]).sort((a, b) => a - b);
  return { fps: times.length / seconds, p95: intervals[Math.floor(intervals.length * 0.95)] };
}

const sixty = sample(60, 144, 10);
assert.ok(sixty.fps >= 59.9 && sixty.fps <= 60.1);
assert.ok(sixty.p95 <= 25);
const ninety = sample(90, 144, 10);
assert.ok(ninety.fps >= 89.9 && ninety.fps <= 90.1);
assert.ok(ninety.p95 <= 16);
console.log("PASS: 60/90 Hz caps preserve cadence on a 144 Hz RAF source");
