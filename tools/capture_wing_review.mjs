// Display-only geometry fixture: real native mesh/pose, controlled display phase.
// This does not inject neural or motor input and is not an autonomous-flight test.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
const output = process.argv[2] ?? 'outputs/indoor-v2/stage-4/wing-review';
const live = process.argv.includes('--live');
const page = await (await fetch('http://127.0.0.1:9337/json/new?http://127.0.0.1:8080/wing-review.html', {method:'PUT'})).json();
const socket = new WebSocket(page.webSocketDebuggerUrl);
await new Promise(resolve => socket.addEventListener('open', resolve, {once:true}));
let next = 0;
const pending = new Map();
socket.onmessage = ({data}) => { const r = JSON.parse(data); const p = pending.get(r.id); if (p) { pending.delete(r.id); r.error ? p.reject(r.error) : p.resolve(r.result); } };
const call = (method, params={}) => new Promise((resolve,reject) => {const id=++next; pending.set(id,{resolve,reject}); socket.send(JSON.stringify({id,method,params}));});
const evaluate = async expression => {
  const result = await call('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});
  if(result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
  return result.result.value;
};
await call('Page.navigate',{url:'http://127.0.0.1:8080/wing-review.html'});
await new Promise(resolve => setTimeout(resolve,500));
await evaluate(`(async()=>{
  const {createSceneRenderer}=await import('/scene.js');
  const {WingDisplayController}=await import('/wing-display.js');
  const canvas=document.createElement('canvas'); canvas.style='position:fixed;left:0;top:0;width:960px;height:540px';
  document.body.append(canvas);
  const renderer=createSceneRenderer(canvas); renderer.resize();
  const ws=new WebSocket('ws://127.0.0.1:8765');
  let latest;
  const frame=await new Promise((resolve,reject)=>{ws.onerror=reject;ws.onmessage=({data})=>{
    if(typeof data!=='string')return;const m=JSON.parse(data);
    if(m.type==='scene')renderer.setScene(m.scene);
    if(m.type==='frame'){latest=m;if(!${live}||String(m.snapshot.flight_mode).toLowerCase()==='cruise'){resolve(m);if(!${live})ws.close();}}
  }});
  renderer.updateFrame(new Float32Array(frame.poses),frame.snapshot);
  const root=frame.snapshot.root_position;
  window.wingReview={renderer,canvas,root,frame,step(view,index){
    if(${live})renderer.updateFrame(new Float32Array(latest.poses),latest.snapshot);
    else if(index===0){renderer.wingDisplay=new WingDisplayController();renderer.wingDisplay.envelope=1;renderer.wingDisplay.ingest({wing_display:{envelope:1,steering:0,physical_frequency_hz:218}});}
    const currentRoot=${live}?latest.snapshot.root_position:root;
    const offset=view==='top'?[0,0,12]:[0,-12,1.5];
    renderer.setObserverView(currentRoot.map((v,i)=>v+offset[i]),currentRoot);
    renderer.render(${live}?performance.now():index*1000/(18*48));
    return {image:canvas.toDataURL('image/png'),wings:renderer.wingPoseDiagnostics(),time:latest.snapshot.time_seconds,mode:latest.snapshot.flight_mode};
  }};
})()`);
const report={kind:live?'live native CNS poses and wing commands; software renderer, not a performance test':'display-only real-mesh fixture; not autonomous behavior',views:{},nativeTimes:{}};
for (const view of ['top','side']) {
  await mkdir(`${output}/${view}`,{recursive:true});
  const samples=[];
  const times=[];
  for(let index=0;index<48;index++) {
    const sample=await evaluate(`window.wingReview.step('${view}',${index})`);
    await writeFile(`${output}/${view}/${String(index).padStart(3,'0')}.png`,Buffer.from(sample.image.split(',')[1],'base64'));
    samples.push(sample.wings);
    times.push({time:sample.time,mode:sample.mode});
  }
  report.views[view]=samples;
  report.nativeTimes[view]=times;
  for(const side of ['left','right']) {
    const wings=samples.flat().filter(w=>w.side===side);
    assert.equal(wings.length,48);
    assert.ok(wings.every(w=>w.anchorErrorMm<1e-6));
    const peak=Math.max(...wings.map(w=>w.openingDegrees));
    assert.ok(peak>=80&&peak<=100,`${side} peak ${peak}`);
  }
}
await writeFile(`${output}/measurements.json`,JSON.stringify(report,null,2)+'\n');
console.log('PASS: both actual mesh spans 90±10 degrees; hinge attachment <1e-6 mm');
await fetch(`http://127.0.0.1:9337/json/close/${page.id}`);
socket.close();
