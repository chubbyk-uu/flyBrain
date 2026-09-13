import test from 'node:test';
import assert from 'node:assert/strict';
import {analyzeReport} from './indoor_long_gate.mjs';
function fixture(position,state={}) {
  const samples=Array.from({length:301},(_,i)=>({time_seconds:i/10,root_position:position(i/10),
    flight_mode:'GROUNDED',contact_count:6,fatigue:0.5,homeostatic_takeoff_inhibited:false,
    homeostatic_resting:false,collision_reflex_active:false,
    cns_motor:{outputs_connected:true,walking_activation:0.5,flight_activation:0.8},
    cns_olfactory:{concentration_ppm:[2,2]},...Object.fromEntries(Object.entries(state).map(([k,v])=>[k,typeof v==='function'?v(i/10):v]))}));
  return {samples,summary:{duration_seconds:30,actual_feeding_events:[],grooming_events:[],
    food_search_recovery_failure_windows:0,invalid_hunger_relief_windows:0},brain:{neurons:166700},
    timebase:{neural_timestep_seconds:0.0001,physics_timestep_seconds:0.0002}};
}
test('a REST or FEED label without physical need change does not hide inactivity',()=>{
  for(const behavior_mode of ['REST','FEED']) {
    assert.ok(analyzeReport(fixture(()=>[0,0,1],{behavior_mode,homeostatic_resting:true})).stationary.length);
  }
});
test('supported stationary fatigue recovery is a real rest',()=>{
  const r=analyzeReport(fixture(()=>[0,0,1],{fatigue:t=>0.9-t*0.02,homeostatic_resting:true,homeostatic_takeoff_inhibited:true}));
  assert.equal(r.stationary.length,0);assert.ok(r.activity.resting.bouts.length);
});
test('repeated real paths are detected but directed movement is not',()=>{
  const circle=analyzeReport(fixture(t=>[10*Math.cos(t*Math.PI/2),10*Math.sin(t*Math.PI/2),20],{flight_mode:'CRUISE',contact_count:0}));
  assert.ok(circle.cycles.some(c=>Math.abs(c.period_seconds-4)<0.01));
  const moving=analyzeReport(fixture(t=>[3*t,0,1]));
  assert.equal(moving.stationary.length,0);assert.equal(moving.cycles.length,0);assert.ok(moving.activity.walking.bouts.length);
});
test('completion count cannot credit an unsupported cleaning action',()=>{
  const r=fixture(t=>[3*t,0,1]);
  r.summary.grooming_events=[{trigger:'autonomous',completed:true,start_seconds:1,end_seconds:3.5,
    minimum_contacts:3,minimum_support_legs:2,rubbing_distance_mm:0.3,head_eye_distance_mm:0.5}];
  assert.ok(analyzeReport(r).failures.includes('invalid physical grooming received completion credit'));
});
