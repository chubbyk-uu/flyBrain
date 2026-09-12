import { createSceneRenderer } from "./scene.js";
import { PoseBuffer } from "./native-pose-buffer.js";
import { decodeRetinaPreview, drawRetinaPreview } from "./native-retina.js";
import { FrameLimiter } from "./frame-limiter.js";

const canvas = document.querySelector("#scene-canvas");
const connection = document.querySelector("#connection");
const metrics = document.querySelector("#metrics");
const retinaCanvas = document.querySelector("#retina-canvas");
const retinaStatus = document.querySelector("#retina-status");
const renderer = createSceneRenderer(canvas, { onError: showError });
let poses = null;
let latest = null;
let renderedFrames = 0;
let fpsStarted = performance.now();
let lastRenderedAt = null;
let reconnectTimer = null;
const acceptance = {
  connected: false, reconnects: 0, streamFrames: 0, renderedFrames: 0,
  frameIntervalsMs: [], latestSnapshot: null, lastError: null,
};
window.__flybrainViewerMetrics = acceptance;
window.flybrainAcceptance = () => {
  const sorted = acceptance.frameIntervalsMs.slice().sort((a, b) => a - b);
  const p95 = sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * 0.95))] ?? null;
  return { ...acceptance, frameIntervalsMs: undefined, p95InterframeMs: p95 };
};

const params = new URLSearchParams(location.search);
const renderHz = Math.min(90, Math.max(20, Number(params.get("fps")) || 60));
const frameLimiter = new FrameLimiter(renderHz);
const socketUrl = params.get("ws") ?? `ws://${location.hostname || "localhost"}:8765`;
function connectSocket() {
  const socket = new WebSocket(socketUrl);
  socket.binaryType = "arraybuffer";
  socket.addEventListener("open", () => {
    acceptance.connected = true;
    connection.classList.remove("error");
    connection.textContent = `connected · ${socketUrl}`;
  });
  socket.addEventListener("close", () => {
    acceptance.connected = false;
    connection.textContent = "disconnected · display neutral; retrying independently";
    renderer.disconnectTelemetry();
    clearTimeout(reconnectTimer);
    reconnectTimer = setTimeout(() => { acceptance.reconnects += 1; connectSocket(); }, 1000);
  });
  socket.addEventListener("error", () => showError(`WebSocket connection failed: ${socketUrl}`));
  socket.addEventListener("message", ({ data }) => {
  if (data instanceof ArrayBuffer) {
    const preview = decodeRetinaPreview(data);
    drawRetinaPreview(retinaCanvas, preview);
    retinaStatus.textContent = `native processed retina · frame ${preview.sequence}`;
    return;
  }
  const message = JSON.parse(data);
  if (message.type === "scene") {
    renderer.setScene(message.scene);
    poses = new PoseBuffer(Number(message.scene.bodyCount) || 0);
    connection.textContent = `scene ready · ${message.scene.brain?.backend ?? "no brain"}`;
  } else if (message.type === "frame" && poses) {
    poses.push(message, performance.now());
    latest = message;
    acceptance.streamFrames += 1;
    acceptance.latestSnapshot = message.snapshot;
  }
  });
}
connectSocket();

function animate(now) {
  if (frameLimiter.due(now)) {
    const frame = poses?.sample(now);
    if (frame) renderer.updateFrame(frame.poses, frame.snapshot);
    renderer.render();
    renderedFrames += 1;
    acceptance.renderedFrames += 1;
    if (lastRenderedAt !== null) {
      acceptance.frameIntervalsMs.push(now - lastRenderedAt);
      if (acceptance.frameIntervalsMs.length > 36_000) acceptance.frameIntervalsMs.shift();
    }
    lastRenderedAt = now;
  }
  if (now - fpsStarted >= 1000) {
    const fps = renderedFrames * 1000 / (now - fpsStarted);
    const snapshot = latest?.snapshot;
    metrics.textContent = snapshot
      ? `${fps.toFixed(1)} FPS (cap ${renderHz}) · stream ${latest.sequence} · sim ${snapshot.time_seconds.toFixed(2)} s · ${snapshot.realtime_factor.toFixed(2)}× · ${snapshot.behavior_mode}/${snapshot.flight_mode} · wing ${renderer.wingDisplayState.physicalFrequencyHz.toFixed(0)} Hz neural / ${renderer.wingDisplayState.displayFrequencyHz.toFixed(0)} Hz display`
      : `${fps.toFixed(1)} FPS · waiting for frames`;
    renderedFrames = 0;
    fpsStarted = now;
  }
  requestAnimationFrame(animate);
}

function showError(error) {
  acceptance.lastError = error instanceof Error ? error.message : String(error);
  connection.textContent = acceptance.lastError;
  connection.classList.add("error");
}

window.addEventListener("resize", () => renderer.resize());
renderer.resize();
requestAnimationFrame(animate);
