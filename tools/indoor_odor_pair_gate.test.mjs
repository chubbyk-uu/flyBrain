import test from 'node:test';
import assert from 'node:assert/strict';
import {analyzePair} from './indoor_odor_pair_gate.mjs';
function pair(){
  const normal={runtime_sha256:'same',summary:{initial_state_sha256:'same',duration_seconds:300},
    sensory_interventions:[],samples:Array.from({length:301},(_,t)=>({time_seconds:t,root_position:[t,0,0],
      cns_olfactory:{band_rate_hz:[[1,1]],spike_delta:1}}))};
  const pulse=structuredClone(normal);
  pulse.sensory_interventions=[{resource:'sugar_drop',odor_enabled:false,time_seconds:2},{resource:'sugar_drop',odor_enabled:true,time_seconds:12}];
  for(const s of pulse.samples.filter(s=>s.time_seconds>=2)){s.root_position[1]=10;s.cns_olfactory.band_rate_hz=[[2,2]];s.cns_olfactory.spike_delta=2;}
  return [normal,pulse];
}
test('registered odor-only response has unchanged pre-intervention state and measured neural change',()=>{
  const r=analyzePair(...pair());assert.equal(r.passed,true);assert.equal(r.trajectory_rms_mm,10);
});
test('unrelated initial differences, pre-intervention motion, and absent neural response fail',()=>{
  for(const mutate of [p=>p.summary.initial_state_sha256='different',p=>p.samples[0].root_position[0]=1,
    p=>p.samples.forEach(s=>{s.cns_olfactory={band_rate_hz:[[1,1]],spike_delta:1};}),p=>p.samples.pop()]){
    const [a,b]=pair();mutate(b);assert.equal(analyzePair(a,b).passed,false);
  }
});
