import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {spawn} from 'node:child_process';
import {analyzeReport} from './indoor_long_gate.mjs';
const [output,selected='all',mode='connected']=process.argv.slice(2);
if(!output||!['connected','odor-pulse'].includes(mode))throw Error('usage: run_indoor_long.mjs OUTPUT [all|SEED] [connected|odor-pulse]');
const seeds=selected==='all'?[11,13,17,19,23]:[Number(selected)];
if(seeds.some(s=>![11,13,17,19,23].includes(s)))throw Error('unregistered seed');
await mkdir(output,{recursive:true});
const results=[];
for(const seed of seeds) {
  const path=`${output}/seed-${seed}.json`;
  const args=['cns-check','--scene','indoor-v2','--duration-seconds','300','--behavior-seed',String(seed),
    '--initial-position-mm','26','-12','32.1','--initial-yaw-deg','0','--initial-hunger','0.72','--initial-dirt','0.1','--output',path];
  if(mode==='connected')args.push('--record-display');
  else args.push('--odor-pulse-resource','sugar_drop','--odor-pulse-start-seconds','2','--odor-pulse-end-seconds','12');
  const child=spawn('target/release/flybrain-world',args,{env:{...process.env,LD_LIBRARY_PATH:`${process.cwd()}/work/mujoco/lib`},stdio:['ignore','ignore','inherit']});
  const code=await new Promise((resolve,reject)=>{child.on('error',reject);child.on('exit',resolve);});
  if(code!==0)throw Error(`seed ${seed} process exited ${code}`);
  const report=JSON.parse(await readFile(path));
  const gate=analyzeReport(report);
  await writeFile(`${output}/seed-${seed}-gate.json`,JSON.stringify(gate,null,2)+'\n',{flag:'wx'});
  const result={seed,mode,passed:gate.passed,failures:gate.failures,meals:gate.meals.map(e=>({resource:e.resource,time:e.validated_at_seconds})),
    grooming_count:gate.grooming.length,invalid_grooming_events:gate.invalid_grooming_events,
    runtime_sha256:report.runtime_sha256,initial_state_sha256:report.summary.initial_state_sha256,
    sensory_interventions:report.sensory_interventions};
  results.push(result);console.log(JSON.stringify(result));
  await writeFile(`${output}/results.json`,JSON.stringify(results,null,2)+'\n');
}
