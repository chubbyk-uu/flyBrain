import assert from "node:assert/strict";
import { decodeRetinaPreview } from "./native-retina.js";
for (const version of [1, 2]) {
  const header = version === 1 ? 16 : 24;
  const bytes = new Uint8Array(header + 8);
  bytes.set(new TextEncoder().encode(`FBR${version}`));
  const view = new DataView(bytes.buffer);
  view.setUint16(4, 4, true); view.setUint16(6, 2, true);
  view.setBigUint64(8, 42n, true);
  if (version === 2) view.setBigUint64(16, 7n, true);
  bytes.fill(55, header);
  const decoded = decodeRetinaPreview(bytes.buffer);
  assert.equal(decoded.epoch, version === 2 ? 7 : null);
  assert.equal(decoded.sequence, 42n);
  assert.equal(decoded.pixels.length, 8);
  assert.throws(() => decodeRetinaPreview(bytes.buffer.slice(0, -1)));
}
assert.throws(() => decodeRetinaPreview(new ArrayBuffer(3)));
console.log("PASS: legacy and epoch-tagged native retina frames, malformed payload rejection");
