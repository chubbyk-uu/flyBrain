export class FrameLimiter {
  constructor(rateHz) {
    this.periodMs = 1000 / Math.max(1, Number(rateHz) || 60);
    this.nextMs = null;
  }

  due(nowMs) {
    const now = Number(nowMs) || 0;
    if (this.nextMs === null) this.nextMs = now;
    if (now + 1e-6 < this.nextMs) return false;
    do this.nextMs += this.periodMs;
    while (this.nextMs <= now);
    return true;
  }
}
