import {readFile,mkdir,writeFile} from 'node:fs/promises';
import {spawn} from 'node:child_process';
const output=process.argv[2];
if(!output)throw new Error('usage: node tools/run_indoor_foraging.mjs OUTPUT [SEED|all] [TASK|all] [motor-off|odor-off]');
const protocol=JSON.parse(await readFile('assets/neuromechfly/scenes/indoor-v2-foraging-tasks.json'));
const seeds=process.argv[3]&&process.argv[3]!=='all'?[Number(process.argv[3])]:protocol.seeds;
const tasks=process.argv[4]&&process.argv[4]!=='all'?protocol.tasks.filter(t=>t.id===process.argv[4]):protocol.tasks;
const ablation=process.argv[5]??'connected';
if(!['connected','motor-off','odor-off'].includes(ablation))throw new Error('unknown ablation');
if(!tasks.length)throw new Error('unknown task');
await mkdir(output,{recursive:true});
const results=[];
for(const task of tasks)for(const seed of seeds) {
  const path=`${output}/${task.id}-${seed}.json`;
  const args=['cns-check','--scene','indoor-v2','--duration-seconds',String(protocol.duration_seconds),
    '--initial-position-mm',...task.position_mm.map(String),'--initial-yaw-deg',String(task.yaw_deg),
    '--initial-hunger',String(protocol.initial_hunger),'--initial-dirt',String(protocol.initial_grooming_urge),
    '--behavior-seed',String(seed),'--output',path];
  if(task.disable_resource)args.push('--disable-resource',task.disable_resource);
  if(ablation==='motor-off')args.push('--disconnect-motor-outputs');
  if(ablation==='odor-off')args.push('--disconnect-olfactory-evoked-inputs');
  console.log(`RUN ${task.id} seed=${seed}`);
  const child=spawn('target/release/flybrain-world',args,{env:{...process.env,LD_LIBRARY_PATH:`${process.cwd()}/work/mujoco/lib`},stdio:['ignore','ignore','inherit']});
  const code=await new Promise((resolve,reject)=>{child.on('error',reject);child.on('exit',resolve);});
  if(code!==0)throw new Error(`run failed: ${task.id}/${seed} exit ${code}`);
  const report=JSON.parse(await readFile(path));
  const events=report.summary.actual_feeding_events??[];
  const event=events.find(e=>!task.required_resource||e.resource===task.required_resource);
  const passed=Boolean(event&&event.validated_at_seconds<=protocol.duration_seconds);
  const result={task:task.id,seed,passed,first_validated:event??null,
    initial_taste:report.samples[0].taste_active,final_position:report.summary.final_position_mm,
    flight_seconds:report.summary.flight_seconds,runtime_sha256:report.runtime_sha256,
    initial_state_sha256:report.summary.initial_state_sha256,
    recovery_failure_windows:report.summary.food_search_recovery_failure_windows,
    recovery_failure_samples:report.samples.filter(s=>s.food_search.recovery_failed).length};
  results.push(result);console.log(JSON.stringify(result));
  await writeFile(`${output}/results.json`,JSON.stringify({protocol,ablation,results},null,2)+'\n');
}
