import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {gunzipSync} from 'node:zlib';
const [connected,motorOff,odorOff]=process.argv.slice(2);
if(!odorOff)throw new Error('usage: summarize_foraging_gate.mjs CONNECTED MOTOR_OFF ODOR_OFF');
const protocol=JSON.parse(await readFile('assets/neuromechfly/scenes/indoor-v2-foraging-tasks.json'));
const load=async path=>{
  const bytes=await readFile(path).catch(async e=>{if(e.code!=='ENOENT')throw e;return gunzipSync(await readFile(path+'.gz'));});
  return {report:JSON.parse(bytes),sha256:createHash('sha256').update(bytes).digest('hex')};
};
const distance=(a,b)=>Math.hypot(...a.map((v,i)=>v-b[i]));
const pathLength=samples=>samples.slice(1).reduce((sum,s,i)=>sum+(samples[i].time_seconds>=0.5?distance(s.root_position,samples[i].root_position):0),0);
function trajectoryRms(a,b) {
  let j=0,sum=0,count=0;
  for(const sample of a) {
    const t=sample.time_seconds;if(t<0.5)continue;
    while(j+1<b.length&&b[j+1].time_seconds<t)j++;
    if(j+1>=b.length)break;
    const left=b[j],right=b[j+1];
    const mix=(t-left.time_seconds)/(right.time_seconds-left.time_seconds);
    const point=left.root_position.map((v,i)=>v+(right.root_position[i]-v)*mix);
    sum+=distance(sample.root_position,point)**2;count++;
  }
  assert.ok(count>100);return Math.sqrt(sum/count);
}
const tasks=[];let runtime;
for(const task of protocol.tasks) {
  const runs=[];
  for(const seed of protocol.seeds) {
    const artifacts=await Promise.all([connected,motorOff,odorOff].map(dir=>load(`${dir}/${task.id}-${seed}.json`)));
    const [normal,motor,odor]=artifacts.map(a=>a.report);
    runtime??=normal.runtime_sha256;
    for(const r of [normal,motor,odor]) {
      assert.equal(r.runtime_sha256,runtime,'mixed runtime versions');
      assert.equal(r.summary.initial_state_sha256,normal.summary.initial_state_sha256,'unpaired initial states');
      assert.equal(r.brain.neurons,166700);
      assert.equal(r.samples[0].taste_active,false);
      assert.equal(r.timebase.neural_timestep_seconds,0.0001);
      assert.equal(r.timebase.physics_timestep_seconds,0.0002);
      assert.equal(r.timebase.control_period_seconds,0.002);
      assert.equal(r.initial_state.initial_hunger,protocol.initial_hunger);
      assert.equal(r.initial_state.parameters.homeostasis.initial_fatigue,protocol.initial_fatigue);
    }
    assert.equal(motor.brain.motor_outputs_connected,false);
    assert.equal(odor.brain.olfactory_evoked_inputs_connected,false);
    assert.equal(normal.brain.motor_outputs_connected,true);
    const event=r=>r.summary.actual_feeding_events.find(e=>(!task.required_resource||e.resource===task.required_resource)&&e.validated_at_seconds<=20);
    const normalEvent=event(normal),odorEvent=event(odor);
    assert.equal(motor.summary.actual_feeding_events.length,0,'motor disconnected still feeds');
    const normalPath=pathLength(normal.samples),motorPath=pathLength(motor.samples);
    assert.ok(motorPath<=normalPath*0.10,`${task.id}/${seed}: motor path ${motorPath}/${normalPath}`);
    const rms=trajectoryRms(normal.samples,odor.samples);
    assert.ok(rms>5,`${task.id}/${seed}: odor ablation trajectory unchanged`);
    assert.equal(normal.summary.food_search_recovery_failure_windows,0,'recovery exceeds five seconds');
    assert.ok(normal.summary.maximum_command_speed_mm_s<=100+1e-6);
    assert.ok(normal.summary.maximum_command_acceleration_mm_s2<=250+1e-6);
    assert.ok(normal.summary.cruise_speed_p95_mm_s<=120);
    assert.ok(normal.summary.maximum_speed_mm_s<=150);
    runs.push({seed,passed:Boolean(normalEvent),first_validated:normalEvent??null,
      odor_off_passed:Boolean(odorEvent),connected_path_mm:normalPath,motor_off_path_mm:motorPath,
      odor_off_trajectory_rms_mm:rms,
      olfactory_readout_spikes:[normal,odor].map(r=>r.summary.olfactory_readout_spikes),
      motor_spikes:[normal,motor,odor].map(r=>r.summary.motor_output_spikes),
      cruise_p95_mm_s:normal.summary.cruise_speed_p95_mm_s,peak_speed_mm_s:normal.summary.maximum_speed_mm_s,
      initial_state_sha256:normal.summary.initial_state_sha256,raw_sha256:artifacts.map(a=>a.sha256)});
  }
  const successes=runs.filter(r=>r.passed).length,odorSuccesses=runs.filter(r=>r.odor_off_passed).length;
  assert.ok(successes>=4,`${task.id}: ${successes}/5`);
  assert.ok(successes-odorSuccesses>=2,`${task.id}: odor ablation does not impair success`);
  tasks.push({task:task.id,successes,odor_off_successes:odorSuccesses,runs});
}
await writeFile(`${connected}/gate.json`,JSON.stringify({passed:true,runtime_sha256:runtime,protocol,tasks},null,2)+'\n');
console.log(tasks.map(t=>`${t.task}: ${t.successes}/5; odor-off ${t.odor_off_successes}/5`).join('\n'));
