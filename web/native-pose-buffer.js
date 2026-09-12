export function interpolatePoses(previous, current, alpha, bodyCount) {
  const left = previous instanceof Float32Array ? previous : new Float32Array(previous ?? []);
  const right = current instanceof Float32Array ? current : new Float32Array(current ?? []);
  if (left.length !== right.length) return right.slice();
  const amount = Math.max(0, Math.min(1, Number(alpha) || 0));
  const output = new Float32Array(right.length);
  const bodyWords = Math.min(right.length, Math.max(0, bodyCount) * 7);
  for (let body = 0; body * 7 < bodyWords; body += 1) {
    const offset = body * 7;
    for (let axis = 0; axis < 3; axis += 1) {
      output[offset + axis] = left[offset + axis] + (right[offset + axis] - left[offset + axis]) * amount;
    }
    slerpWxyz(left, right, output, offset + 3, amount);
  }
  // Camera position and rotation rows are display metadata. Linear blending is
  // sufficient here; scene.js orthonormalizes them when constructing a quaternion.
  for (let index = bodyWords; index < right.length; index += 1) {
    output[index] = left[index] + (right[index] - left[index]) * amount;
  }
  return output;
}

function slerpWxyz(left, right, output, offset, amount) {
  let aw = left[offset];
  let ax = left[offset + 1];
  let ay = left[offset + 2];
  let az = left[offset + 3];
  let bw = right[offset];
  let bx = right[offset + 1];
  let by = right[offset + 2];
  let bz = right[offset + 3];
  let dot = aw * bw + ax * bx + ay * by + az * bz;
  if (dot < 0) {
    dot = -dot;
    bw = -bw; bx = -bx; by = -by; bz = -bz;
  }
  let leftScale = 1 - amount;
  let rightScale = amount;
  if (dot < 0.9995) {
    const angle = Math.acos(Math.max(-1, Math.min(1, dot)));
    const sine = Math.sin(angle);
    leftScale = Math.sin((1 - amount) * angle) / sine;
    rightScale = Math.sin(amount * angle) / sine;
  }
  const values = [
    aw * leftScale + bw * rightScale,
    ax * leftScale + bx * rightScale,
    ay * leftScale + by * rightScale,
    az * leftScale + bz * rightScale,
  ];
  const norm = Math.hypot(...values) || 1;
  for (let index = 0; index < 4; index += 1) output[offset + index] = values[index] / norm;
}

export class PoseBuffer {
  constructor(bodyCount) {
    this.bodyCount = bodyCount;
    this.previous = null;
    this.current = null;
  }

  push(message, receivedAt) {
    const frame = { ...message, poses: new Float32Array(message.poses ?? []), receivedAt };
    if (!this.current || frame.epoch !== this.current.epoch) {
      this.previous = frame;
      this.current = frame;
      return;
    }
    if (frame.sequence <= this.current.sequence) return;
    this.previous = this.current;
    this.current = frame;
  }

  sample(now) {
    if (!this.current) return null;
    if (!this.previous || this.previous === this.current) return this.current;
    const period = Math.max(1, this.current.receivedAt - this.previous.receivedAt);
    const alpha = Math.max(0, Math.min(1, (now - this.current.receivedAt) / period));
    return {
      ...this.current,
      poses: interpolatePoses(this.previous.poses, this.current.poses, alpha, this.bodyCount),
      snapshot: interpolateSnapshot(this.previous.snapshot, this.current.snapshot, alpha),
    };
  }
}

export function interpolateSnapshot(previous, current, alpha) {
  if (!previous || !current) return current ?? previous ?? null;
  const amount = Math.max(0, Math.min(1, Number(alpha) || 0));
  const snapshot = { ...current };
  if (Array.isArray(previous.root_position) && Array.isArray(current.root_position)) {
    snapshot.root_position = current.root_position.map((value, index) => {
      const left = Number(previous.root_position[index]);
      const right = Number(value);
      return Number.isFinite(left) && Number.isFinite(right) ? left + (right - left) * amount : right;
    });
  }
  snapshot.time_seconds = Number(previous.time_seconds)
    + (Number(current.time_seconds) - Number(previous.time_seconds)) * amount;
  return snapshot;
}
