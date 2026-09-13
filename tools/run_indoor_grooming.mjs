import {readFile,mkdir,writeFile} from 'node:fs/promises';
import {spawn} from 'node:child_process';
const [output,mode='connected']=process.argv.slice(2);
if(!output||!['connected','probe-off','motor-off'].includes(mode))throw new Error('usage: run_indoor_grooming.mjs OUTPUT [connected|probe-off|motor-off]');
await mkdir(output,{recursive:true});
const results=[];
for(const [surface,position] of [['floor',[0,-70,2.1]],['table',[0,0,32.1]]]) {
  const path=`${output}/${surface}-11.json`;
  const args=['cns-check','--scene','indoor-v2','--duration-seconds','12','--behavior-seed','11',
    '--initial-position-mm',...position.map(String),'--initial-hunger','0.3','--initial-dirt','0.9',
    '--disable-resource','sugar_drop','--disable-resource','flower_nectar','--output',path];
  if(mode==='connected')args.push('--record-display');
  if(mode==='probe-off')args.push('--disconnect-grooming-probe');
  if(mode==='motor-off')args.push('--disconnect-motor-outputs');
  const child=spawn('target/release/flybrain-world',args,{env:{...process.env,LD_LIBRARY_PATH:`${process.cwd()}/work/mujoco/lib`},stdio:['ignore','ignore','inherit']});
  const code=await new Promise((resolve,reject)=>{child.on('error',reject);child.on('exit',resolve);});
  if(code!==0)throw new Error(`grooming ${surface} exited ${code}`);
  const r=JSON.parse(await readFile(path));
  const event=r.summary.grooming_events.find(e=>e.completed&&e.trigger==='autonomous');
  const passed=Boolean(event&&event.start_seconds-event.opportunity_seconds<=2
    &&Math.abs(event.end_seconds-event.start_seconds-4)<=0.003
    &&event.minimum_contacts>=4&&event.minimum_support_legs>=4
    &&event.rubbing_distance_mm!==null&&event.rubbing_distance_mm<1.5
    &&event.head_eye_distance_mm!==null&&event.head_eye_distance_mm<1.5);
  const result={surface,mode,passed,event:event??null,events:r.summary.grooming_events,
    invalid_hunger_relief_windows:r.summary.invalid_hunger_relief_windows,
    runtime_sha256:r.runtime_sha256,initial_state_sha256:r.summary.initial_state_sha256};
  results.push(result);console.log(JSON.stringify(result));
  await writeFile(`${output}/results.json`,JSON.stringify(results,null,2)+'\n');
}
