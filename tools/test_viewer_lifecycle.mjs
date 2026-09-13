// Integration probe: lifecycle-only commands on a dedicated native test server.
import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { decodeRetinaPreview } from "../web/native-retina.js";
const endpoint = process.argv[2] ?? "ws://127.0.0.1:8765";
const output = process.argv[3] ?? "outputs/indoor-v2/stage-3/websocket-lifecycle.json";
const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
async function until(test, label) {
  const started = performance.now();
  while (!test()) {
    assert.ok(performance.now() - started < 20000, `timeout: ${label}`);
    await delay(20);
  }
}
async function client() {
  const socket = new WebSocket(endpoint);
  socket.binaryType = "arraybuffer";
  const state = { socket, frame: null, retinaEpochs: [], rejected: 0 };
  socket.addEventListener("message", ({data}) => {
    if (data instanceof ArrayBuffer) {
      const preview = decodeRetinaPreview(data);
      state.retinaEpochs.push(preview.epoch);
      if (state.frame && preview.epoch !== state.frame.epoch) state.rejected += 1;
    } else {
      const message = JSON.parse(data);
      if (message.type === "frame") state.frame = message;
    }
  });
  await until(() => state.frame, "diagnostic receives frames without ready");
  state.send = (command) => socket.send(JSON.stringify({type:"control", command}));
  return state;
}
const events = [];
function record(label, c) {
  const s = c.frame.snapshot;
  events.push({label, epoch:c.frame.epoch, time:s.time_seconds, paused:s.paused,
    hunger:s.hunger, flight_fatigue:s.flight_fatigue, grooming_urge:s.grooming_urge});
}
const a = await client();
await until(() => a.frame.snapshot.time_seconds >= 5, "background advances before ready");
assert.equal(a.frame.epoch,0);
record("diagnostic did not reset", a);
const b = await client();
a.send("viewer_ready"); b.send("viewer_ready");
await until(() => a.frame.epoch === 1 && b.frame.epoch === 1, "simultaneous ready resets once");
assert.equal(a.frame.snapshot.paused,false);
record("first ready",a);
b.socket.close();
const c = await client(); c.send("viewer_ready");
await delay(500);
assert.equal(c.frame.epoch,1); record("new tab / reconnect",c);
a.send("pause");
await until(() => a.frame.snapshot.paused, "pause");
const frozen = {...a.frame.snapshot};
await delay(5000);
for (const key of ["time_seconds","hunger","flight_fatigue","grooming_urge","cumulative_spiking_neuron_count"]) {
  assert.equal(a.frame.snapshot[key],frozen[key],key);
}
record("paused five wall seconds",a);
a.send("reset");
await until(() => a.frame.epoch === 2,"manual reset");
assert.equal(a.frame.snapshot.paused,true);
assert.equal(a.frame.snapshot.time_seconds,0);
assert.equal(a.frame.snapshot.hunger,0.72);
assert.deepEqual(a.frame.snapshot.root_position,[26,-12,32.1]);
record("manual reset paused",a);
c.send("viewer_ready");
await delay(500);
assert.equal(a.frame.epoch,2); assert.equal(a.frame.snapshot.paused,true);
a.send("resume");
await until(() => a.frame.snapshot.time_seconds > .1,"resume");
record("continued",a);
assert.ok(a.retinaEpochs.length > 0, "native retina must be present");
assert.equal(a.rejected,0, "publisher must not send mismatched retina epochs");
a.socket.close();c.socket.close();
const report = {passed:true, events, retinaFrames:a.retinaEpochs.length, retinaEpochs:[...new Set(a.retinaEpochs)], staleRetinaFrames:a.rejected};
await mkdir(output.slice(0,output.lastIndexOf("/")),{recursive:true});
await writeFile(output, JSON.stringify(report,null,2)+"\n");
console.log(JSON.stringify(report,null,2));
