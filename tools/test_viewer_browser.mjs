import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
const output = "outputs/indoor-v2/stage-3/browser";
const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const page = await (await fetch("http://127.0.0.1:9337/json/new?about:blank", {method:"PUT"})).json();
const socket = new WebSocket(page.webSocketDebuggerUrl);
await new Promise(resolve => socket.addEventListener("open",resolve,{once:true}));
const pending = new Map(); let sequence = 0;
socket.addEventListener("message", ({data}) => {
  const r = JSON.parse(data); const p = pending.get(r.id);
  if (p) {pending.delete(r.id); clearTimeout(p.timer); r.error ? p.reject(r.error) : p.resolve(r.result);}
});
function call(method, params = {}) {
  return new Promise((resolve,reject) => {
    const id = ++sequence;
    const timer = setTimeout(()=>{pending.delete(id);reject(new Error(`CDP timeout ${method}`));},30000);
    pending.set(id,{resolve,reject,timer}); socket.send(JSON.stringify({id,method,params}));
  });
}
async function evaluate(expression) {
  const r = await call("Runtime.evaluate",{expression,returnByValue:true});
  if (r.exceptionDetails) throw new Error(r.result.description);
  return r.result.value;
}
async function until(test,label) {
  const started = performance.now();
  while (!(await test())) {assert.ok(performance.now()-started < 30000,`timeout ${label}`);await delay(200);}
}
await call("Emulation.setDeviceMetricsOverride",{width:1600,height:1000,deviceScaleFactor:1,mobile:false});
// Backend is independently started; allow a genuine pre-run before the formal page.
await delay(5000);
await call("Page.navigate",{url:"http://127.0.0.1:8080/native-view.html"});
await until(async()=> (await evaluate("window.flybrainAcceptance?.()"))?.epoch===1,"formal viewer auto-reset");
const first = await evaluate("window.flybrainAcceptance()");
assert.equal(first.latestSnapshot.paused,false);
await evaluate("document.querySelector('#pause').click()");
await until(async()=> (await evaluate("window.flybrainAcceptance()"))?.latestSnapshot.paused,"pause button");
await call("Page.reload",{ignoreCache:true});
await until(async()=> (await evaluate("window.flybrainAcceptance?.()"))?.latestSnapshot?.paused,"reload stays paused");
assert.equal((await evaluate("window.flybrainAcceptance()")).epoch,1);
await evaluate("document.querySelector('#reset').click()");
await until(async()=> (await evaluate("window.flybrainAcceptance()"))?.epoch===2,"reset button");
const reset = await evaluate("window.flybrainAcceptance()");
assert.equal(reset.latestSnapshot.time_seconds,0); assert.equal(reset.latestSnapshot.paused,true);
await evaluate("document.querySelector('#overview').click()");
await delay(500);
await mkdir(output,{recursive:true});
const screenshot = await call("Page.captureScreenshot",{format:"png"});
await writeFile(`${output}/reset-paused-overview.png`,Buffer.from(screenshot.data,"base64"));
const other = await (await fetch("http://127.0.0.1:9337/json/new?http://127.0.0.1:8080/native-view.html",{method:"PUT"})).json();
await delay(4000);
assert.equal((await evaluate("window.flybrainAcceptance()")).epoch,2);
assert.equal((await evaluate("window.flybrainAcceptance()")).latestSnapshot.paused,true);
await fetch(`http://127.0.0.1:9337/json/close/${other.id}`);
await evaluate("document.querySelector('#resume').click()");
await until(async()=> (await evaluate("window.flybrainAcceptance()")).latestSnapshot.time_seconds>.1,"resume button");
const resumed = await evaluate("window.flybrainAcceptance()");
assert.equal(resumed.epoch,2);
assert.equal(resumed.lastError,null);
const report = {passed:true,firstEpoch:first.epoch,resetEpoch:reset.epoch,resetTime:reset.latestSnapshot.time_seconds,
  resumedTime:resumed.latestSnapshot.time_seconds,needsLabel:await evaluate("document.querySelector('#needs').textContent"),
  note:"Linux Chromium/SwiftShader functional UI evidence; not Windows performance acceptance"};
await writeFile(`${output}/result.json`,JSON.stringify(report,null,2)+"\n");
console.log(JSON.stringify(report));
await call("Page.navigate",{url:"about:blank"});
socket.close();
