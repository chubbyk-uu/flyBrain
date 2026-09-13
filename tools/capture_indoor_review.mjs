// Capture rendered acceptance cards using an already running local Chromium CDP.
import { mkdir, writeFile } from "node:fs/promises";
const output = process.argv[2] ?? "outputs/indoor-v2/stage-1";
const indices = process.argv.includes("--retina") ? [13, 14] : [0,1,2,3,4,5,6,7];
const pages = await (await fetch("http://127.0.0.1:9337/json/list")).json();
const socket = new WebSocket(pages.find((page) => page.type === "page").webSocketDebuggerUrl);
await new Promise((resolve) => socket.addEventListener("open", resolve, { once: true }));
let sequence = 0;
const requests = new Map();
socket.addEventListener("message", ({ data }) => {
  const response = JSON.parse(data);
  const pending = requests.get(response.id);
  if (pending) { requests.delete(response.id); response.error ? pending.reject(response.error) : pending.resolve(response.result); }
});
function call(method, params = {}) {
  return new Promise((resolve, reject) => {
    const id = ++sequence;
    requests.set(id, { resolve, reject });
    socket.send(JSON.stringify({ id, method, params }));
  });
}
await call("Emulation.setDeviceMetricsOverride", { width: 1600, height: 1100, deviceScaleFactor: 1, mobile: false });
await call("Page.navigate", { url: "http://127.0.0.1:8080/native-gallery.html" });
let cards = [];
for (let attempt = 0; attempt < 45; attempt += 1) {
  await new Promise((resolve) => setTimeout(resolve, 1000));
  const result = await call("Runtime.evaluate", {
    expression: `${JSON.stringify(indices)}.map(i => document.querySelectorAll('figure')[i]).map(f => ({label:f.querySelector('figcaption').textContent,src:Boolean(f.querySelector('img'))}))`, returnByValue: true,
  });
  cards = result.result.value ?? [];
  if (cards.length === indices.length && cards.every((card) => card.src)) break;
}
if (cards.length !== indices.length || cards.some((card) => !card.src)) throw new Error("Missing review images");
await mkdir(output, { recursive: true });
for (let i = 0; i < cards.length; i += 1) {
  const result = await call("Runtime.evaluate", {expression: `document.querySelectorAll('figure')[${indices[i]}].querySelector('img').src`, returnByValue:true});
  await writeFile(`${output}/${i}-${cards[i].label.split(" · ")[0].replaceAll(/[^a-zA-Z0-9]+/g, "-")}.png`, Buffer.from(result.result.value.split(",")[1], "base64"));
}
const screenshot = await call("Page.captureScreenshot", { format: "png" });
await writeFile(`${output}/gallery.png`, Buffer.from(screenshot.data, "base64"));
console.log(JSON.stringify(cards.map(({label}) => label)));
socket.close();
