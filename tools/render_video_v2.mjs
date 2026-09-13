import {readFile,writeFile,mkdir} from 'node:fs/promises';import {spawn} from 'node:child_process';
const out=process.env.FLYBRAIN_VIDEO_OUT??'outputs/social-video-v2',old='outputs/social-video-20260913';
const layout=process.argv[2]??'portrait',wide=layout==='landscape',only=process.argv[3];if(!['portrait','landscape'].includes(layout))throw Error('layout');
await mkdir(`${out}/${layout}/shots`,{recursive:true});
const target=await(await fetch('http://127.0.0.1:9338/json/new?http://127.0.0.1:8080/film-studio-v2.html',{method:'PUT'})).json();
const ws=new WebSocket(target.webSocketDebuggerUrl);await new Promise(r=>ws.addEventListener('open',r,{once:true}));let seq=0;const pending=new Map();
ws.onmessage=({data})=>{const r=JSON.parse(data),p=pending.get(r.id);if(p){pending.delete(r.id);r.error?p.reject(Error(JSON.stringify(r.error))):p.resolve(r.result);}};
ws.onclose=()=>{for(const p of pending.values())p.reject(Error('CDP closed'));};
const call=(method,params={})=>new Promise((resolve,reject)=>{const id=++seq;pending.set(id,{resolve,reject});ws.send(JSON.stringify({id,method,params}));});
const ev=async expression=>{const r=await call('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails));return r.result.value;};
await call('Emulation.setDeviceMetricsOverride',{width:wide?1920:1080,height:wide?1080:1920,deviceScaleFactor:1,mobile:false});
for(let i=0;i<40;i++){if(await ev('window.filmReady'))break;await new Promise(r=>setTimeout(r,500));}
if(!await ev('window.filmReady'))throw Error('Renderer did not initialize');
const shots=JSON.parse(await readFile(process.env.FLYBRAIN_VIDEO_SHOTS??`${out}/shots.json`)),narration=JSON.parse(await readFile(`${out}/narration.json`));
for(const shot of shots){if(only&&shot.name!==only)continue;if(shot.reuse)continue;
 const input=shot.input??(shot.source==='stereo'?`${out}/sources/stereo-telemetry/take.json`:`${old}/sources/${shot.source}.json`);
 const raw=JSON.parse(await readFile(input));
 const selected=raw.display_replay.frames.filter(f=>f.snapshot.time_seconds>=shot.start-(shot.view==='neural'?10:.05)&&f.snapshot.time_seconds<=shot.start+shot.span+.05);
 if(shot.source==='stereo')for(const f of selected){
  if(f.left_time!==f.right_time||f.left_time!==f.snapshot.time_seconds)throw Error('Stereo desynchronization');
  if(!(f.snapshot.filtered_population_rate_hz>0))throw Error('Missing neural telemetry');
  if(shot.view==='retina')f.stereo_base64=(await readFile(`${out}/sources/stereo-telemetry/${f.retina_file}`)).toString('base64');
 }
 const payload=JSON.stringify({display_replay:{scene:raw.display_replay.scene,frames:selected}});
 await ev('window.filmChunks=[]');for(let i=0;i<payload.length;i+=4_000_000)await ev(`filmChunks.push(${JSON.stringify(payload.slice(i,i+4_000_000))})`);
 await ev(`(async()=>{await filmLoad(JSON.parse(filmChunks.join('')),${JSON.stringify(shot)},${wide});filmChunks=null;return true})()`);
 const duration=shot.duration??narration.find(r=>r.name===shot.name).duration,n=Math.round(duration*30),path=`${out}/${layout}/shots/${shot.name}`;
 if(process.argv.includes('--still')){await writeFile(path+'-review.jpg',Buffer.from(await ev('filmFrame(.45)'),'base64'));continue;}
 const ff=spawn('ffmpeg',['-v','error','-y','-f','image2pipe','-vcodec','mjpeg','-framerate','30','-i','pipe:0','-an','-c:v','libx264','-preset','fast','-crf','18','-pix_fmt','yuv420p',path+'.mp4'],{stdio:['pipe','ignore','inherit']});
 const done=new Promise((resolve,reject)=>ff.on('exit',c=>c?reject(Error('ffmpeg '+c)):resolve()));
 for(let i=0;i<n;i++){
  const data=Buffer.from(await ev(`filmFrame(${i/n})`),'base64');if(i===Math.floor(n*.45))await writeFile(path+'-review.jpg',data);
  if(!ff.stdin.write(data))await new Promise(r=>ff.stdin.once('drain',r));if(i%90===0)console.log(layout,shot.name,i,n);
 }
 ff.stdin.end();await done;console.log('DONE',layout,shot.name,duration);
}
ws.close();await fetch(`http://127.0.0.1:9338/json/close/${target.id}`);
