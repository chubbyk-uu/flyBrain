// Read-only diagnostic subscriber: never sends viewer_ready or control commands.
import {writeFile,mkdir} from 'node:fs/promises';
import {dirname} from 'node:path';
const [output,duration='600',endpoint='ws://127.0.0.1:8765']=process.argv.slice(2);
if(!output||!Number.isFinite(Number(duration))||Number(duration)<=0)throw Error('usage: record_native_snapshots.mjs OUTPUT WALL_SECONDS [WS_URL]');
const socket=new WebSocket(endpoint),samples=[];
let brain=null,last=-Infinity,lastEpoch=null,frames=0,finished=false;
const start=performance.now();
socket.onmessage=({data})=>{
  if(typeof data!=='string')return;
  const message=JSON.parse(data);
  if(message.type==='scene')brain=message.scene.brain;
  if(message.type!=='frame')return;
  frames++;
  if(message.epoch!==lastEpoch||message.snapshot.time_seconds-last>=0.1-1e-6){
    samples.push({wall_seconds:(performance.now()-start)/1000,epoch:message.epoch,...message.snapshot});
    last=message.snapshot.time_seconds;lastEpoch=message.epoch;
  }
};
async function finish(reason){
  if(finished)return;finished=true;socket.close();
  await mkdir(dirname(output),{recursive:true});
  await writeFile(output,JSON.stringify({kind:'read-only live snapshot diagnostic, not full physical acceptance trace',
    reason,brain,wall_seconds:(performance.now()-start)/1000,frames,samples},null,2)+'\n',{flag:'wx'});
  console.log(JSON.stringify({output,reason,frames,samples:samples.length,first:samples[0],last:samples.at(-1)}));
  process.exit(0);
}
socket.onerror=()=>finish('socket error');
socket.onclose=()=>finish('socket closed');
process.on('SIGINT',()=>finish('operator stopped diagnostic'));
process.on('SIGTERM',()=>finish('operator stopped diagnostic'));
setTimeout(()=>finish('duration completed'),Number(duration)*1000);
