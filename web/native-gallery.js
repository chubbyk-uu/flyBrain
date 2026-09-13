import { createSceneRenderer } from "./scene.js";
import { PoseBuffer } from "./native-pose-buffer.js";
import { decodeRetinaPreview, drawRetinaPreview } from "./native-retina.js";

const definitions = [
  ["room", "Indoor room"], ["north", "North view"], ["south", "South view"],
  ["east", "East view"], ["west", "West view"],
  ["table", "Coffee table"], ["plant", "Potted flower"], ["candy", "Candy"],
  ["grounded", "Grounded fly"], ["walking", "Walking"], ["flight", "CNS-gated flight"],
  ["feeding", "Sugar / nectar feeding"], ["grooming", "Leg and head/eye grooming"],
  ["retina-left", "Native retina · left"], ["retina-right", "Native retina · right"],
];
const cards = new Map();
const cardsRoot = document.querySelector("#cards");
for (const [key, label] of definitions) {
  const figure = document.createElement("figure");
  figure.innerHTML = `<div class="pending">waiting for ${label}</div><figcaption>${label}</figcaption>`;
  cardsRoot.append(figure);
  cards.set(key, figure);
}

const status = document.querySelector("#status");
const canvas = document.querySelector("#capture");
const renderer = createSceneRenderer(canvas, { onError: (error) => { status.textContent = String(error); status.className = "warn"; } });
let poses;
let latest;
let captureBusy = false;
let indoorV2 = false;
const captured = new Set();

function store(key, dataUrl, detail) {
  if (captured.has(key) || !dataUrl?.startsWith("data:image/png") || dataUrl.length < 1000) return false;
  const card = cards.get(key);
  card.querySelector(".pending")?.remove();
  const image = document.createElement("img");
  image.src = dataUrl;
  image.alt = card.querySelector("figcaption").textContent;
  card.prepend(image);
  card.querySelector("figcaption").textContent += ` · ${detail}`;
  captured.add(key);
  status.textContent = `${captured.size}/${definitions.length} non-empty acceptance views captured`;
  status.className = captured.size === definitions.length ? "ok" : "";
  return true;
}

async function captureView(key, position, target, detail) {
  renderer.setObserverView(position, target);
  renderer.render();
  await new Promise(requestAnimationFrame);
  renderer.render();
  store(key, canvas.toDataURL("image/png"), detail);
}

async function captureFixedViews() {
  if (captureBusy || captured.has("room") || !latest) return;
  captureBusy = true;
  try {
    if (indoorV2) {
      for (const [key, position, target] of [
        ["room", [240, -300, 230], [0, 0, 35]],
        ["north", [0, 290, 155], [0, 0, 35]],
        ["south", [0, -290, 155], [0, 0, 35]],
        ["east", [330, 0, 170], [0, 0, 35]],
        ["west", [-330, 0, 170], [0, 0, 35]],
        ["table", [165, -180, 125], [0, 0, 25]],
        ["plant", [0, -57, 95], [-48, 12, 49]],
        ["candy", [65, -37, 49], [48, -12, 30]],
      ]) await captureView(key, position, target, "indoor-v2 display-only camera");
      renderer.setCameraMode("chase");
      return;
    }
    await captureView("room", [560, -590, 330], [0, 0, 70], `t=${latest.snapshot.time_seconds.toFixed(1)}s`);
    await captureView("table", [270, -70, 125], [120, 70, 48], "display-only camera");
    await captureView("plant", [286, -218, 105], [220, -130, 35], "display-only camera");
    renderer.setCameraMode("chase");
  } finally {
    captureBusy = false;
  }
}

function behaviorKey(snapshot) {
  if (snapshot.grooming_active) return "grooming";
  if (snapshot.taste_active || Number(snapshot.feeding_extension) > 0.1) return "feeding";
  if (String(snapshot.flight_mode).toLowerCase() !== "grounded") return "flight";
  if (String(snapshot.behavior_mode).toLowerCase().includes("walk")) return "walking";
  return "grounded";
}

async function captureBehavior() {
  if (captureBusy || !latest) return;
  const key = behaviorKey(latest.snapshot);
  if (captured.has(key)) return;
  captureBusy = true;
  try {
    renderer.setCameraMode("chase");
    renderer.updateFrame(poses.sample(performance.now()).poses, latest.snapshot);
    renderer.render();
    await new Promise(requestAnimationFrame);
    renderer.render();
    store(key, canvas.toDataURL("image/png"), `native t=${latest.snapshot.time_seconds.toFixed(1)}s`);
  } finally {
    captureBusy = false;
  }
}

const params = new URLSearchParams(location.search);
const socket = new WebSocket(params.get("ws") ?? `ws://${location.hostname || "localhost"}:8765`);
socket.binaryType = "arraybuffer";
socket.addEventListener("open", () => { status.textContent = "Connected; collecting only observed native states…"; });
socket.addEventListener("close", () => { renderer.disconnectTelemetry(); status.textContent += " · stream disconnected"; });
socket.addEventListener("message", ({ data }) => {
  if (data instanceof ArrayBuffer) {
    const preview = decodeRetinaPreview(data);
    const pair = document.createElement("canvas");
    drawRetinaPreview(pair, preview);
    for (let eye = 0; eye < 2; eye += 1) {
      const crop = document.createElement("canvas");
      crop.width = pair.width / 2; crop.height = pair.height;
      crop.getContext("2d").drawImage(pair, eye * crop.width, 0, crop.width, crop.height, 0, 0, crop.width, crop.height);
      store(eye ? "retina-right" : "retina-left", crop.toDataURL("image/png"), `native frame ${preview.sequence}`);
    }
    return;
  }
  const message = JSON.parse(data);
  if (message.type === "scene") {
    indoorV2 = message.scene.geoms.some((geom) => String(geom.material).startsWith("indoor-v2/"));
    renderer.setScene(message.scene);
    poses = new PoseBuffer(Number(message.scene.bodyCount) || 0);
  } else if (message.type === "frame" && poses) {
    poses.push(message, performance.now()); latest = message;
    const frame = poses.sample(performance.now());
    renderer.updateFrame(frame.poses, frame.snapshot);
    captureFixedViews().then(captureBehavior);
  }
});

function animate() {
  const frame = poses?.sample(performance.now());
  if (!captureBusy) {
    if (frame) renderer.updateFrame(frame.poses, frame.snapshot);
    renderer.render();
  }
  // This is an evidence gallery, not the live viewer. Avoid an unbounded render
  // queue competing with its own synchronous capture on software review hosts.
  setTimeout(() => requestAnimationFrame(animate), 100);
}
requestAnimationFrame(animate);
window.addEventListener("resize", () => renderer.resize());
