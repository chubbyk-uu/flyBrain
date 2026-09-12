import { createSceneRenderer } from "./scene.js";
import { PoseBuffer } from "./native-pose-buffer.js";
import { decodeRetinaPreview, drawRetinaPreview } from "./native-retina.js";

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

const params = new URLSearchParams(location.search);
const renderHz = Math.min(90, Math.max(20, Number(params.get("fps")) || 60));
const renderPeriodMs = 1000 / renderHz;
let nextRenderAt = 0;
const socketUrl = params.get("ws") ?? `ws://${location.hostname || "localhost"}:8765`;
const socket = new WebSocket(socketUrl);
socket.binaryType = "arraybuffer";
socket.addEventListener("open", () => { connection.textContent = `connected · ${socketUrl}`; });
socket.addEventListener("close", () => { connection.textContent = "disconnected"; });
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
  }
});

function animate(now) {
  if (now >= nextRenderAt) {
    const frame = poses?.sample(now);
    if (frame) renderer.updateFrame(frame.poses, frame.snapshot);
    renderer.render();
    renderedFrames += 1;
    nextRenderAt = now + renderPeriodMs;
  }
  if (now - fpsStarted >= 1000) {
    const fps = renderedFrames * 1000 / (now - fpsStarted);
    const snapshot = latest?.snapshot;
    metrics.textContent = snapshot
      ? `${fps.toFixed(1)} FPS (cap ${renderHz}) · stream ${latest.sequence} · sim ${snapshot.time_seconds.toFixed(2)} s · ${snapshot.realtime_factor.toFixed(2)}× · ${snapshot.behavior_mode}/${snapshot.flight_mode}`
      : `${fps.toFixed(1)} FPS · waiting for frames`;
    renderedFrames = 0;
    fpsStarted = now;
  }
  requestAnimationFrame(animate);
}

function showError(error) {
  connection.textContent = error instanceof Error ? error.message : String(error);
  connection.classList.add("error");
}

window.addEventListener("resize", () => renderer.resize());
renderer.resize();
requestAnimationFrame(animate);
