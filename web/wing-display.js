const TAU = Math.PI * 2;

function clamp(value, minimum, maximum) {
  return Math.min(maximum, Math.max(minimum, Number(value) || 0));
}

export function wingSignal(snapshot) {
  const signal = snapshot?.wing_display;
  const grounded = String(snapshot?.flight_mode ?? "Grounded").toLowerCase() === "grounded";
  const fallbackEnvelope = grounded ? 0 : clamp(snapshot?.brain_flight_drive, 0, 1);
  return {
    envelope: clamp(signal?.envelope ?? fallbackEnvelope, 0, 1),
    steering: clamp(signal?.steering ?? snapshot?.brain_flight_steering, -1, 1),
    physicalFrequencyHz: clamp(signal?.physical_frequency_hz ?? (grounded ? 0 : 218), 0, 1000),
    phaseCycles: ((Number(signal?.phase_cycles) || 0) % 1 + 1) % 1,
    connected: Boolean(signal),
    paused: Boolean(snapshot?.paused),
  };
}

// The physical 218 Hz carrier cannot be sampled faithfully by a 60/90 Hz monitor.
// This controller renders a forward-only 18 Hz symbolic carrier whose amplitude and
// left/right balance come from the CNS flight readout. It is display-only.
export class WingDisplayController {
  constructor() {
    this.phaseCycles = 0;
    this.envelope = 0;
    this.lastNowMs = null;
    this.signal = wingSignal(null);
  }

  ingest(snapshot) {
    this.signal = wingSignal(snapshot);
    if (this.lastNowMs === null) this.phaseCycles = this.signal.phaseCycles;
  }

  sample(nowMs) {
    const now = Number(nowMs) || 0;
    const dt = this.lastNowMs === null || this.signal.paused ? 0 : clamp((now - this.lastNowMs) / 1000, 0, 0.1);
    this.lastNowMs = now;
    const response = 1 - Math.exp(-dt / 0.045);
    this.envelope += (this.signal.envelope - this.envelope) * response;
    const displayFrequencyHz = Math.min(18, this.signal.physicalFrequencyHz);
    this.phaseCycles = (this.phaseCycles + displayFrequencyHz * dt) % 1;
    const carrier = Math.sin(TAU * this.phaseCycles);
    const asymmetry = 0.12 * this.signal.steering;
    const leftEnvelope = clamp(this.envelope * (1 - asymmetry), 0, 1);
    const rightEnvelope = clamp(this.envelope * (1 + asymmetry), 0, 1);
    return {
      phaseCycles: this.phaseCycles,
      physicalFrequencyHz: this.signal.physicalFrequencyHz,
      displayFrequencyHz,
      connected: this.signal.connected,
      leftEnvelope,
      rightEnvelope,
      // Opening measured from the posterior body axis, not a local joint swing.
      // Folded 15 degrees; flight stroke 50..90 degrees at full neural envelope.
      leftAngleRad: (15 + (55 + 20 * carrier) * leftEnvelope) * Math.PI / 180,
      rightAngleRad: (15 + (55 + 20 * carrier) * rightEnvelope) * Math.PI / 180,
      blurOpacity: 0.12 + 0.22 * this.envelope,
    };
  }
}
