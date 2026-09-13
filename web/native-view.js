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
let activeSocket = null;
let readyPending = false;
let currentEpoch = null;
const needs = document.querySelector("#needs");
const runState = document.querySelector("#run-state");
function sendControl(command) {
  if (activeSocket?.readyState === WebSocket.OPEN) activeSocket.send(JSON.stringify({ type: "control", command }));
}
for (const command of ["pause", "resume", "reset"]) {
  document.querySelector(`#${command}`).addEventListener("click", () => sendControl(command));
}
document.querySelector("#overview").addEventListener("click", () => renderer.setObserverView([240,-300,230],[0,0,35]));
document.querySelector("#follow").addEventListener("click", () => renderer.setCameraMode("chase"));
const acceptance = {
  connected: false, reconnects: 0, streamFrames: 0, renderedFrames: 0,
  frameIntervalsMs: [], latestSnapshot: null, lastError: null,
  mainRenderFrames: 0, mainRenderDurationsMs: [], freshPoseFrames: 0, retinaFrames: 0,
  lastFreshPoseAt: null, maxPoseAgeMs: 0, poseBufferFrames: 0,
  visibilityChanges: [],
};
document.addEventListener('visibilitychange',()=>acceptance.visibilityChanges.push({at:performance.now(),state:document.visibilityState}));
window.__flybrainViewerMetrics = acceptance;
window.flybrainAcceptance = () => {
  const sorted = acceptance.frameIntervalsMs.slice().sort((a, b) => a - b);
  const p95 = sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * 0.95))] ?? null;
  const gl=renderer.renderer.getContext();
  const debug=gl.getExtension('WEBGL_debug_renderer_info');
  return { ...acceptance, frameIntervalsMs: undefined, mainRenderDurationsMs: undefined,
    p95InterframeMs: p95, visibility:document.visibilityState, focused:document.hasFocus(),
    shadowEnabled:renderer.renderer.shadowMap.enabled,shadowMapSize:renderer.keyLight.shadow.mapSize.toArray(),
    ceilingLightEnabled:renderer.ceilingLight.visible,ceilingShadowMapSize:renderer.ceilingLight.shadow.mapSize.toArray(),
    glRenderer:debug?gl.getParameter(debug.UNMASKED_RENDERER_WEBGL):gl.getParameter(gl.RENDERER) };
};

const params = new URLSearchParams(location.search);
const renderHz = Math.min(90, Math.max(20, Number(params.get("fps")) || 60));
const frameLimiter = new FrameLimiter(renderHz);
const socketUrl = params.get("ws") ?? `ws://${location.hostname || "localhost"}:8765`;
function connectSocket() {
  const socket = new WebSocket(socketUrl);
  activeSocket = socket;
  socket.binaryType = "arraybuffer";
  socket.addEventListener("open", () => {
    acceptance.connected = true;
    if (acceptance.lastError?.startsWith("WebSocket connection failed:")) acceptance.lastError = null;
    connection.classList.remove("error");
    connection.textContent = `connected · ${socketUrl}`;
  });
  socket.addEventListener("close", () => {
    acceptance.connected = false;
    connection.textContent = "连接已断开 · 正在重连";
    connection.classList.add("error");
    runState.textContent = "旧画面已冻结；当前仿真状态未知，请勿将此画面视为正在飞行或卡住。";
    renderer.disconnectTelemetry();
    clearTimeout(reconnectTimer);
    reconnectTimer = setTimeout(() => { acceptance.reconnects += 1; connectSocket(); }, 1000);
  });
  socket.addEventListener("error", () => showError(`WebSocket connection failed: ${socketUrl}`));
  socket.addEventListener("message", ({ data }) => {
  if (data instanceof ArrayBuffer) {
    const preview = decodeRetinaPreview(data);
    if (preview.epoch !== currentEpoch) return;
    drawRetinaPreview(retinaCanvas, preview);
    acceptance.retinaFrames += 1;
    retinaStatus.textContent = `native processed retina · frame ${preview.sequence}`;
    return;
  }
  const message = JSON.parse(data);
  if (message.type === "scene") {
    acceptance.sceneBrain=message.scene.brain;
    renderer.setScene(message.scene);
    poses = new PoseBuffer(Number(message.scene.bodyCount) || 0);
    latest = null;
    currentEpoch = null;
    readyPending = true;
    connection.textContent = `scene ready · ${message.scene.brain?.backend ?? "no brain"}`;
  } else if (message.type === "frame" && poses) {
    if (message.epoch !== currentEpoch) {
      currentEpoch = message.epoch;
      renderer.resetTelemetry();
      retinaCanvas.getContext("2d").clearRect(0, 0, retinaCanvas.width, retinaCanvas.height);
      retinaStatus.textContent = "等待当前 epoch 的 native 双眼画面…";
    }
    const arrivedAt=performance.now();
    if(!latest||message.epoch!==latest.epoch||message.snapshot.time_seconds>latest.snapshot.time_seconds) {
      acceptance.freshPoseFrames+=1;
      acceptance.lastFreshPoseAt=arrivedAt;
    }
    poses.push(message, arrivedAt);
    acceptance.poseBufferFrames=poses.previous===poses.current?1:2;
    latest = message;
    acceptance.streamFrames += 1;
    acceptance.latestSnapshot = message.snapshot;
    acceptance.epoch = message.epoch;
    const s = message.snapshot;
    const percent = (value) => `${(100 * Number(value ?? 0)).toFixed(0)}%`;
    needs.textContent = `饥饿 ${percent(s.hunger)} · 飞行疲劳 ${percent(s.flight_fatigue)} · 清洁冲动 ${percent(s.grooming_urge)}`;
    const search = s.food_search?.retreating ? "沿已走路径退开" : s.food_search?.escaping_overhang ? "离开上方遮挡" : s.food_search?.vertical_sampling ? "高度采样" : s.food_search?.recovery_active ? "无进展恢复" : s.food_search?.turning_back ? "回查感觉较强处" : "无额外搜索干预";
    runState.textContent = `${s.paused ? "已暂停" : "运行中"} · seed ${s.behavior_seed ?? 0} · epoch ${message.epoch} · 起飞限制：${s.takeoff_inhibited_reason || "none"} · ${search}`;
  }
  });
}
connectSocket();

function animate(now) {
  if (frameLimiter.due(now)) {
    const frame = poses?.sample(now);
    if (frame && acceptance.connected) renderer.updateFrame(frame.poses, frame.snapshot);
    const renderStarted=performance.now();
    const beforeMainFrame=renderer.renderer.info.render.frame;
    renderer.render();
    acceptance.mainRenderFrames+=renderer.renderer.info.render.frame-beforeMainFrame;
    acceptance.mainRenderDurationsMs.push(performance.now()-renderStarted);
    if(acceptance.mainRenderDurationsMs.length>36_000)acceptance.mainRenderDurationsMs.shift();
    if(acceptance.lastFreshPoseAt!==null)acceptance.maxPoseAgeMs=Math.max(acceptance.maxPoseAgeMs,performance.now()-acceptance.lastFreshPoseAt);
    if (readyPending && frame) {
      readyPending = false;
      // Only the formal viewer sends ready, after scene and first pose rendered.
      sendControl("viewer_ready");
    }
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
    metrics.textContent = snapshot && !acceptance.connected
      ? `已断连 · 最后收到的仿真时间 ${snapshot.time_seconds.toFixed(2)} s（不是实时画面）`
      : snapshot
      ? `${fps.toFixed(1)} FPS (cap ${renderHz}) · stream ${latest.sequence} · sim ${snapshot.time_seconds.toFixed(2)} s · ${snapshot.realtime_factor.toFixed(2)}× · ${snapshot.behavior_mode}/${snapshot.flight_mode} · wing ${renderer.wingDisplayState.physicalFrequencyHz.toFixed(0)} Hz physical command / ${renderer.wingDisplayState.displayFrequencyHz.toFixed(0)} Hz display`
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
