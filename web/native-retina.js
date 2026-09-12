const HEADER_BYTES = 16;

export function decodeRetinaPreview(buffer) {
  const bytes = new Uint8Array(buffer);
  if (bytes.length < HEADER_BYTES || String.fromCharCode(...bytes.subarray(0, 4)) !== "FBR1") {
    throw new Error("unknown native retina message");
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const width = view.getUint16(4, true);
  const height = view.getUint16(6, true);
  const sequence = view.getBigUint64(8, true);
  const pixels = bytes.subarray(HEADER_BYTES);
  if (pixels.length !== width * height) throw new Error("native retina payload has the wrong size");
  return { width, height, sequence, pixels };
}

export function drawRetinaPreview(canvas, preview) {
  const rgba = new Uint8ClampedArray(preview.pixels.length * 4);
  for (let index = 0; index < preview.pixels.length; index += 1) {
    const target = index * 4;
    const intensity = preview.pixels[index];
    rgba[target] = intensity;
    rgba[target + 1] = intensity;
    rgba[target + 2] = intensity;
    rgba[target + 3] = 255;
  }
  canvas.width = preview.width;
  canvas.height = preview.height;
  canvas.getContext("2d", { alpha: false }).putImageData(
    new ImageData(rgba, preview.width, preview.height), 0, 0,
  );
}
