import assert from "node:assert/strict";
import { WingDisplayController, wingSignal } from "./wing-display.js";

assert.equal(wingSignal({ flight_mode: "Grounded", brain_flight_drive: 1 }).envelope, 0);
assert.deepEqual(wingSignal({ wing_display: { envelope: 2, steering: -3, physical_frequency_hz: 218, phase_cycles: 1.25 } }), {
  envelope: 1, steering: -1, physicalFrequencyHz: 218, phaseCycles: 0.25, connected: true,
});

const controller = new WingDisplayController();
controller.ingest({ wing_display: { envelope: 1, steering: 0.8, physical_frequency_hz: 218, phase_cycles: 0 } });
const phases = [];
let final;
for (let frame = 0; frame < 180; frame += 1) {
  final = controller.sample(frame * 1000 / 60);
  phases.push(final.phaseCycles);
}
assert.ok(final.leftEnvelope < final.rightEnvelope, "positive steering must strengthen the right display wing");
assert.ok(final.leftEnvelope > 0.7 && final.rightEnvelope <= 1);
for (let i = 1; i < phases.length; i += 1) {
  const advance = (phases[i] - phases[i - 1] + 1) % 1;
  assert.ok(advance > 0 && advance < 0.5, "60 FPS display phase must advance without reverse/freeze aliasing");
}

controller.ingest({ flight_mode: "Grounded", brain_flight_drive: 0 });
for (let frame = 180; frame < 240; frame += 1) final = controller.sample(frame * 1000 / 60);
assert.ok(final.leftEnvelope < 0.1 && final.rightEnvelope < 0.1);

const disconnected = new WingDisplayController();
disconnected.ingest({ flight_mode: "Cruise", brain_flight_drive: 0.6, brain_flight_steering: -0.5 });
for (let frame = 0; frame < 60; frame += 1) final = disconnected.sample(frame * 1000 / 60);
assert.equal(final.connected, false);
assert.ok(final.leftEnvelope > final.rightEnvelope, "legacy telemetry must degrade to a live neutral-compatible signal");
console.log("PASS: grounded fold, continuous carrier, CNS steering asymmetry and legacy telemetry fallback");
