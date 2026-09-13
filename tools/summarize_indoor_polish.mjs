// Current-layout short regression; deliberately NOT a 300-second acceptance.
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
const [foraging,grooming,output]=process.argv.slice(2);
if(!output)throw Error('usage: summarize_indoor_polish.mjs FORAGING_DIR GROOM_PARENT OUTPUT');
const json=async path=>JSON.parse(await readFile(path));
const hash=async path=>createHash('sha256').update(await readFile(path)).digest('hex');
const runtime=await hash('target/release/flybrain-world');
const scene=await hash('assets/neuromechfly/indoor-v2.xml');
const habitat=await hash('assets/neuromechfly/scenes/indoor-v2-habitat.json');
const causal=await json(`${grooming}/grooming-gate.json`);
assert.equal(causal.passed,true,'paired grooming causal gates');
const protocol=await json('assets/neuromechfly/scenes/indoor-v2-foraging-tasks.json');
let pack=null;
function identity(r){
  assert.equal(r.runtime_sha256,runtime);
  assert.equal(r.initial_state.assets['indoor-v2.xml'],scene,'stale scene');
  assert.equal(r.initial_state.assets['scene-habitat:indoor-v2'],habitat,'stale habitat');
  assert.equal(r.brain.neurons,166700);
  assert.equal(r.timebase.neural_timestep_seconds,0.0001);
  assert.equal(r.timebase.physics_timestep_seconds,0.0002);
  pack??=r.initial_state.pack_arrays;
  assert.deepEqual(r.initial_state.pack_arrays,pack);
  assert.equal(r.summary.invalid_hunger_relief_windows,0);
}
const meals=[];
for(const task of protocol.tasks){
  const file=`${foraging}/${task.id}-11.json`,r=await json(file);identity(r);
  const event=r.summary.actual_feeding_events.find(e=>!task.required_resource||e.resource===task.required_resource);
  assert(event&&event.validated_at_seconds<=20&&event.continuous_seconds>=0.2&&event.hunger_after<event.hunger_before,task.id);
  meals.push({task:task.id,file,event});
}
const rub=[];
for(const surface of ['floor','table']){
  for(const mode of ['connected','probe-off','motor-off']){
    const file=`${grooming}/groom-${mode}/${surface}-11.json`,r=await json(file);identity(r);
    if(mode!=='connected'){assert.equal(r.summary.grooming_events.length,0);continue;}
    const samples=r.samples.filter(s=>s.grooming_active&&s.grooming_phase>=0.2625&&s.grooming_phase<=0.6375);
    const distances=samples.map(s=>s.front_tarsi_distance_mm);
    const span=samples.at(-1).time_seconds-samples[0].time_seconds;
    const excursion=Math.max(...distances)-Math.min(...distances);
    assert(span>=1.48&&span<=1.52,'inserted rubbing duration');
    assert(Math.max(...distances)<1.5&&excursion>0.04,'actual paired-foot rubbing motion');
    assert(samples.every(s=>s.grooming_actual_support_leg_count>=4),'actual support during rubbing');
    rub.push({surface,file,inserted_rub_seconds:span,tarsi_distance_excursion_mm:excursion,
      minimum_distance_mm:Math.min(...distances),maximum_distance_mm:Math.max(...distances)});
  }
}
const report={passed:true,kind:'one-seed short physical/causal regression, not multi-seed long-course acceptance',
  runtime_sha256:runtime,scene_sha256:scene,habitat_sha256:habitat,pack_arrays:pack,meals,rub,causal};
await writeFile(output,JSON.stringify(report,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({passed:true,meals:meals.map(m=>({task:m.task,seconds:m.event.validated_at_seconds})),rub}));
