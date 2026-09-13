// Visual review of recorded native CNS poses, never an injected/manual action.
import {readFile,mkdir,writeFile} from 'node:fs/promises';
const [input,output,start,end]=process.argv.slice(2);
if(!input||!output)throw Error('usage: capture_grooming_replay.mjs REPORT OUTPUT [START END]');
const report=JSON.parse(await readFile(input));
const segment=start!==undefined;
if(segment&&(!Number.isFinite(Number(start))||!Number.isFinite(Number(end))||Number(start)<0||Number(end)<=Number(start)||Number(end)>report.summary.duration_seconds))throw Error('invalid native replay interval');
const event=segment?{start_seconds:Number(start),end_seconds:Number(end),trigger:'recorded diagnostic interval'}:
  report.summary.grooming_events.find(e=>e.completed&&e.trigger==='autonomous');
if(!event||!report.display_replay)throw Error('Missing autonomous completed event or native pose recording');
const frames=report.display_replay.frames.filter(f=>f.snapshot.time_seconds>=event.start_seconds-0.1&&f.snapshot.time_seconds<=event.end_seconds+0.1);
// A fixed world-side camera can look straight at the abdomen after a turn.
// Align the review camera once with the actual thorax heading at segment start.
const thorax=report.display_replay.scene.bodies.findIndex(b=>b.name==='fly/c_thorax');
if(thorax<0||!frames.length)throw Error('Missing thorax or replay frames');
const [qw,qx,qy,qz]=frames[0].poses.slice(thorax*7+3,thorax*7+7);
const heading=[1-2*(qy*qy+qz*qz),2*(qx*qy+qw*qz)];
const headingNorm=Math.hypot(...heading)||1;
const sideOffset=[9*heading[1]/headingNorm,-9*heading[0]/headingNorm,1.5];
const frontOffset=[7*heading[0]/headingNorm,7*heading[1]/headingNorm,2];
const target=await (await fetch('http://127.0.0.1:9337/json/new?http://127.0.0.1:8080/wing-review.html',{method:'PUT'})).json();
const ws=new WebSocket(target.webSocketDebuggerUrl);
await new Promise(resolve=>ws.addEventListener('open',resolve,{once:true}));
let sequence=0;const pending=new Map();
ws.onmessage=({data})=>{const r=JSON.parse(data);const p=pending.get(r.id);if(p){pending.delete(r.id);r.error?p.reject(r.error):p.resolve(r.result);}};
const call=(method,params={})=>new Promise((resolve,reject)=>{const id=++sequence;pending.set(id,{resolve,reject});ws.send(JSON.stringify({id,method,params}));});
const evaluate=async expression=>{const r=await call('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails));return r.result.value;};
await call('Page.navigate',{url:'http://127.0.0.1:8080/wing-review.html'});
await new Promise(resolve=>setTimeout(resolve,300));
await evaluate(`(async()=>{const {createSceneRenderer}=await import('/scene.js');
document.body.innerHTML='<canvas style="position:fixed;inset:0;width:960px;height:540px"></canvas>';
const canvas=document.querySelector('canvas');const renderer=createSceneRenderer(canvas);renderer.resize();
renderer.setScene(${JSON.stringify(report.display_replay.scene)});
window.review={renderer,canvas,frames:${JSON.stringify(frames)}};})()`);
for(const view of segment?['context','side']:['top','side','front']) {
  await mkdir(`${output}/${view}`,{recursive:true});
  for(let index=0;index<frames.length;index++) {
    const data=await evaluate(`(()=>{const {renderer,canvas,frames}=window.review;const f=frames[${index}];
renderer.updateFrame(new Float32Array(f.poses),f.snapshot);const root=f.snapshot.root_position;
const offset=${JSON.stringify(view==='context'?[45,-65,50]:view==='top'?[0,0,9]:view==='front'?frontOffset:sideOffset)};renderer.setObserverView(root.map((v,i)=>v+offset[i]),root);
renderer.render(f.snapshot.time_seconds*1000);return canvas.toDataURL('image/png');})()`);
    await writeFile(`${output}/${view}/${String(index).padStart(3,'0')}.png`,Buffer.from(data.split(',')[1],'base64'),{flag:'wx'});
  }
}
await writeFile(`${output}/provenance.json`,JSON.stringify({input,runtime_sha256:report.runtime_sha256,
  event,frames:frames.length,source_hz:50,side_camera_offset_mm:sideOffset,kind:'display-only replay of recorded CNS/MuJoCo body poses; not performance evidence'},null,2)+'\n',{flag:'wx'});
await fetch(`http://127.0.0.1:9337/json/close/${target.id}`);ws.close();
console.log(`Saved ${frames.length} native frames per view to ${output}`);
