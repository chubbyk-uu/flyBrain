import assert from "node:assert/strict";
import { PoseBuffer, interpolatePoses, interpolateSnapshot } from "./native-pose-buffer.js";
import { decodeRetinaPreview } from "./native-retina.js";

const left = new Float32Array([0, 0, 0, 1, 0, 0, 0]);
const right = new Float32Array([10, 4, 2, 0, 0, 0, 1]);
const halfway = interpolatePoses(left, right, 0.5, 1);
assert.deepEqual([...halfway.slice(0, 3)], [5, 2, 1]);
assert.ok(Math.abs(Math.hypot(...halfway.slice(3, 7)) - 1) < 1e-6);

const buffer = new PoseBuffer(1);
buffer.push({ sequence: 1, epoch: 0, poses: left, snapshot: { time_seconds: 0 } }, 100);
buffer.push({ sequence: 2, epoch: 0, poses: right, snapshot: { time_seconds: 0.033 } }, 133);
assert.equal(buffer.sample(133).poses[0], 0);
assert.ok(Math.abs(buffer.sample(149.5).poses[0] - 5) < 1e-6);
assert.equal(buffer.sample(166).poses[0], 10);
assert.deepEqual(
  interpolateSnapshot(
    { time_seconds: 1, root_position: [0, 2, 4] },
    { time_seconds: 2, root_position: [10, 6, 8] },
    0.5,
  ),
  { time_seconds: 1.5, root_position: [5, 4, 6] },
);
buffer.push({ sequence: 2, epoch: 0, poses: left, snapshot: { time_seconds: 0.033 } }, 168);
assert.equal(buffer.sample(168).poses[0], 10);
buffer.push({ sequence: 1, epoch: 1, poses: left, snapshot: { time_seconds: 0 } }, 170);
assert.equal(buffer.sample(171).poses[0], 0);
const retinaMessage = new Uint8Array(16 + 450 * 256);
retinaMessage.set([70, 66, 82, 49]);
const retinaHeader = new DataView(retinaMessage.buffer);
retinaHeader.setUint16(4, 450, true);
retinaHeader.setUint16(6, 256, true);
retinaHeader.setBigUint64(8, 42n, true);
retinaMessage.fill(93, 16);
const retina = decodeRetinaPreview(retinaMessage.buffer);
assert.equal(retina.sequence, 42n);
assert.equal(retina.pixels.length, 450 * 256);
assert.equal(retina.pixels[0], 93);
console.log("PASS: native pose interpolation and epoch reset");
