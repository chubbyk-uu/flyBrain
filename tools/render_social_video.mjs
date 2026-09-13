import {readFile,writeFile,mkdir} from 'node:fs/promises';import {spawn} from 'node:child_process';
const out='outputs/social-video-20260913';await mkdir(out+'/shots',{recursive:true});
const target=await(await fetch('http://127.0.0.1:9338/json/new?http://127.0.0.1:8080/film-studio.html',{method:'PUT'})).json();
const ws=new WebSocket(target.webSocketDebuggerUrl);await new Promise(r=>ws.addEventListener('open',r,{once:true}));let seq=0;const pending=new Map();ws.onmessage=({data})=>{let r=JSON.parse(data),p=pending.get(r.id);if(p){pending.delete(r.id);r.error?p.reject(Error(JSON.stringify(r.error))):p.resolve(r.result);}};
const call=(method,params={})=>new Promise((resolve,reject)=>{let id=++seq;pending.set(id,{resolve,reject});ws.send(JSON.stringify({id,method,params}));});
const ev=async(expression)=>{let r=await call('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails));return r.result.value;};
await call('Emulation.setDeviceMetricsOverride',{width:1080,height:1920,deviceScaleFactor:1,mobile:false});
for(let i=0;i<40;i++){if(await ev('window.filmReady'))break;await new Promise(r=>setTimeout(r,500));}
const narration=JSON.parse(await readFile(out+'/narration.json'));const shots=JSON.parse(await readFile(out+'/shots.json'));const live=JSON.parse(await readFile(out+'/sources/live.json'));
const only=process.argv[2];
for(const s of shots){if(only&&s.name!==only)continue;
 const raw=JSON.parse(await readFile(`${out}/sources/${s.source}.json`));
 const r={display_replay:{scene:raw.display_replay.scene,frames:raw.display_replay.frames.filter(f=>f.snapshot.time_seconds>=s.start-.05&&f.snapshot.time_seconds<=s.start+s.span+.05)}};
 const payload=JSON.stringify(r);
 await ev('window.filmChunks=[]');
 for(let i=0;i<payload.length;i+=4_000_000)await ev(`filmChunks.push(${JSON.stringify(payload.slice(i,i+4_000_000))})`);
 await ev('window.filmReport=JSON.parse(filmChunks.join(""));filmChunks=null');
 const slimLive=['neural','retina'].includes(s.view)?{frames:live.frames.map(f=>({snapshot:f.snapshot})),retina:live.retina}:null;
 await ev(`filmLoad(filmReport,${JSON.stringify(s)},${JSON.stringify(slimLive)})`);
 const duration=narration.find(n=>n.name===s.name).duration,frames=Math.ceil(duration*30);
 if(process.argv.includes('--still')){let data=await ev('filmFrame(.45)');await writeFile(`${out}/shots/${s.name}-review.jpg`,Buffer.from(data,'base64'));continue;}
 const ff=spawn('ffmpeg',['-hide_banner','-loglevel','error','-y','-f','image2pipe','-vcodec','mjpeg','-framerate','30','-i','pipe:0','-an','-c:v','libx264','-preset','fast','-crf','18','-pix_fmt','yuv420p',`${out}/shots/${s.name}.mp4`],{stdio:['pipe','ignore','inherit']});
 const done=new Promise((res,rej)=>ff.on('exit',c=>c?rej(Error('ffmpeg '+c)):res()));
 for(let i=0;i<frames;i++){const data=Buffer.from(await ev(`filmFrame(${i/frames})`),'base64');if(i===Math.floor(frames*.45))await writeFile(`${out}/shots/${s.name}-review.jpg`,data);if(!ff.stdin.write(data))await new Promise(r=>ff.stdin.once('drain',r));if(i%90===0)console.log(s.name,i,frames);}
 ff.stdin.end();await done;console.log('DONE',s.name,duration);
}
ws.close();await fetch(`http://127.0.0.1:9338/json/close/${target.id}`);
