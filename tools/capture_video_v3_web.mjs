import {mkdir,writeFile} from 'node:fs/promises';import {spawn} from 'node:child_process';
const out='outputs/social-video-v3/web-capture';await mkdir(out,{recursive:true});
const t=await(await fetch('http://localhost:9338/json/new?http://localhost:8080/native-view.html?ws=ws://127.0.0.1:8776&fps=30',{method:'PUT'})).json();
const ws=new WebSocket(t.webSocketDebuggerUrl);await new Promise(r=>ws.onopen=r);let id=0;const pending=new Map();
ws.onmessage=e=>{const m=JSON.parse(e.data);if(pending.has(m.id)){const p=pending.get(m.id);pending.delete(m.id);m.error?p.reject(Error(JSON.stringify(m.error))):p.resolve(m.result)}};
const call=(method,params={})=>new Promise((resolve,reject)=>{const n=++id;pending.set(n,{resolve,reject});ws.send(JSON.stringify({id:n,method,params}))});
const ev=async expression=>{const r=await call('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails));return r.result.value};
await call('Emulation.setDeviceMetricsOverride',{width:1280,height:720,deviceScaleFactor:1,mobile:false});
for(let i=0;i<100;i++){if(await ev('!!window.flybrainAcceptance'))break;await new Promise(r=>setTimeout(r,300));}
await ev(`(async()=>{const {FlySceneRenderer}=await import('/scene.js');const original=FlySceneRenderer.prototype.render;FlySceneRenderer.prototype.render=function(...args){this.pendingVision=false;this.renderer.toneMappingExposure=1.38;const s=this.snapshot;if(s?.root_position){const p=s.root_position;const i=this.bodyDescriptors.findIndex(b=>b.name==='fly/c_thorax');const q=this.bodyGroups[i]?.quaternion;const yaw=q?Math.atan2(2*(q.x*q.y+q.w*q.z),1-2*(q.y*q.y+q.z*q.z)):0;this.setObserverView([p[0]+9*Math.cos(yaw-1.1),p[1]+9*Math.sin(yaw-1.1),p[2]+3],p)}return original.apply(this,args)};
const style=document.createElement('style');style.textContent='.status{max-width:680px;font-size:12px}.retina,.neural{width:300px}.neural canvas{height:75px}';document.head.append(style);
document.querySelector('.status strong').textContent='我的赛博果蝇 · 运行界面';
document.querySelector('.status span:last-child').textContent='拖动旋转 · 滚轮缩放';
document.querySelector('.neural strong').textContent='神经网络群体活动';
document.querySelector('body > div:last-of-type').textContent='真实神经连接结构 + 物理身体 + 工程规则';
window.requestAnimationFrame=callback=>{window.filmRaf=callback;return 1};
window.filmTick=()=>{window.filmRaf?.(performance.now());const s=window.flybrainAcceptance().latestSnapshot;if(s)document.querySelector('#metrics').textContent='仿真时间 '+s.time_seconds.toFixed(2)+' 秒 · '+(s.behavior_mode==='Feed'?'正在进食':s.grooming_active?'正在清洁':'自由活动')};return true})()`);
let selected;
for(let i=0;i<700;i++){
 await ev('window.filmTick()');
 const s=await ev('window.flybrainAcceptance().latestSnapshot');
 if(i%30===0)console.log('waiting',s?.time_seconds,s?.behavior_mode,s?.taste_active,s?.grooming_active);
 if(s&&(s.grooming_active||(s.taste_active&&s.feeding_extension>.5))){selected=s;break;}
 await new Promise(r=>setTimeout(r,300));
}
if(!selected)throw Error('No feeding or grooming captured; do not fabricate live evidence');
const ff=spawn('ffmpeg',['-v','error','-y','-f','image2pipe','-vcodec','mjpeg','-framerate','30','-i','pipe:0','-an','-c:v','libx264','-preset','fast','-crf','18','-pix_fmt','yuv420p',out+'/live.mp4'],{stdio:['pipe','ignore','inherit']});
const done=new Promise((resolve,reject)=>ff.on('exit',c=>c?reject(Error('ffmpeg '+c)):resolve()));const samples=[];const started=Date.now();
for(let i=0;i<78;i++){
 await ev('window.filmTick()');
 const metrics=await ev('window.flybrainAcceptance()');samples.push({wall_ms:Date.now()-started,metrics});
 const shot=await call('Page.captureScreenshot',{format:'jpeg',quality:95});const bytes=Buffer.from(shot.data,'base64');
 if(i===35)await writeFile(out+'/review.jpg',bytes);
 if(!ff.stdin.write(bytes))await new Promise(r=>ff.stdin.once('drain',r));
 if(i%15===0)console.log('capturing',i,metrics.latestSnapshot.time_seconds,metrics.latestSnapshot.behavior_mode);
}
ff.stdin.end();await done;await writeFile(out+'/capture.json',JSON.stringify({selected,samples},null,2));
console.log('CAPTURED',selected.behavior_mode,selected.grooming_active,samples[0].metrics.latestSnapshot.time_seconds,samples.at(-1).metrics.latestSnapshot.time_seconds);
ws.close();await fetch('http://localhost:9338/json/close/'+t.id);
