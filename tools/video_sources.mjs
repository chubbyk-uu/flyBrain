import {mkdir,writeFile,readFile} from 'node:fs/promises';
import {spawn} from 'node:child_process';
const out='outputs/social-video-20260913';await mkdir(out+'/sources',{recursive:true});
// Capture genuine streamed telemetry and native retinal frames before offline takes.
const ws=new WebSocket('ws://127.0.0.1:8765');ws.binaryType='arraybuffer';
const live={scene:null,frames:[],retina:[]};
ws.onmessage=({data})=>{if(data instanceof ArrayBuffer){live.retina.push({frame:live.frames.length,data:Buffer.from(data).toString('base64')});}else{let m=JSON.parse(data);if(m.type==='scene')live.scene=m.scene;if(m.type==='frame')live.frames.push(m);}};
await new Promise(r=>ws.addEventListener('open',r,{once:true}));
ws.send(JSON.stringify({type:'control',command:'resume'}));
await new Promise(r=>setTimeout(r,12000));
ws.send(JSON.stringify({type:'control',command:'pause'}));await new Promise(r=>setTimeout(r,250));ws.close();
await writeFile(out+'/sources/live.json',JSON.stringify(live));
const runs=[
 ['sugar',15,[47,-19,32.1],0,.9,.1,'flower_nectar'],
 ['flower',16,[-70,-4,32.1],30,.72,.1,'sugar_drop'],
 ['groom',12,[0,0,32.1],0,.3,.9,null],
 ['journey',100,[26,-12,32.1],0,.72,.1,null],
];
for(const [name,duration,pos,yaw,hunger,urge,disable] of runs){
 const args=['cns-check','--scene','indoor-v2','--duration-seconds',String(duration),'--behavior-seed','11','--initial-position-mm',...pos.map(String),'--initial-yaw-deg',String(yaw),'--initial-hunger',String(hunger),'--initial-dirt',String(urge),'--record-display','--output',`${out}/sources/${name}.json`];
 if(disable)args.push('--disable-resource',disable);
 const child=spawn('target/release/flybrain-world',args,{env:{...process.env,LD_LIBRARY_PATH:process.cwd()+'/work/mujoco/lib'},stdio:['ignore','ignore','inherit']});
 await new Promise((r,j)=>{child.on('error',j);child.on('exit',c=>c?j(Error(name+': '+c)):r());});
 const report=JSON.parse(await readFile(`${out}/sources/${name}.json`));
 console.log(name,JSON.stringify({frames:report.display_replay.frames.length,feeding:report.summary.actual_feeding_events,grooming:report.summary.grooming_events}));
}
