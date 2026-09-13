import {readFile,writeFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
import {gunzipSync} from 'node:zlib';
const distance=(a,b)=>Math.hypot(...a.map((x,i)=>x-b[i]));
const mean=xs=>xs.reduce((a,b)=>a+b,0)/Math.max(1,xs.length);
const inEvent=(time,events)=>events.some(e=>time>=e.start_seconds&&time<=e.end_seconds);
const validGroom=e=>e.trigger==='autonomous'&&e.completed&&e.end_seconds-e.start_seconds>=2
  &&e.end_seconds-e.start_seconds<=3&&e.minimum_contacts>=4&&e.minimum_support_legs>=4
  &&e.rubbing_distance_mm!==null&&e.rubbing_distance_mm<1.5&&e.head_eye_distance_mm!==null&&e.head_eye_distance_mm<1.5;

function bouts(points,key,minimum=0) {
  const result=[];let start=null;
  for(let i=0;i<=points.length;i++) {
    if(i<points.length&&points[i][key]) {if(start===null)start=i;}
    else if(start!==null) {
      const end=i-1;const duration=(end-start+1)*0.1;
      if(duration+1e-9>=minimum)result.push({start_seconds:points[start].t,end_seconds:points[end].t+0.1,
        duration_seconds:duration,displacement_mm:distance(points[start].p,points[end].p)});
      start=null;
    }
  }
  return result;
}

export function analyzeReport(report) {
  const samples=report.samples;
  const meals=(report.summary.actual_feeding_events??[]).filter(e=>e.continuous_seconds>=0.2-1e-9&&e.hunger_after<e.hunger_before);
  const grooming=(report.summary.grooming_events??[]).filter(validGroom);
  const points=[];let index=0;
  for(let t=0.1;t<=report.summary.duration_seconds+1e-6;t+=0.1) {
    while(index+1<samples.length&&samples[index+1].time_seconds<=t+1e-6)index++;
    const s=samples[index];const p=s.root_position;
    const previous=points.at(-2);const speed=previous?distance(p,previous.p)/0.2:0;
    const supported=s.flight_mode==='GROUNDED'&&s.contact_count>=3;
    const fatigueDecreasing=previous&&s.fatigue<previous.fatigue-0.00001;
    const feeding=inEvent(t,meals);const clean=inEvent(t,grooming);
    const recovery=supported&&s.homeostatic_takeoff_inhibited&&fatigueDecreasing;
    const resting=recovery&&speed<2&&s.homeostatic_resting;
    const safety=Boolean(s.collision_reflex_active);
    const motor=s.cns_motor?.outputs_connected&&Math.max(s.cns_motor.walking_activation,s.cns_motor.flight_activation)>0.1;
    points.push({t,p,speed,fatigue:s.fatigue,feeding,clean,recovery,resting,safety,
      flying:s.flight_mode!=='GROUNDED',walking:supported&&speed>=2&&!feeding&&!clean,
      eligible:Boolean(motor&&!feeding&&!clean&&!recovery&&!safety),
      concentration:mean(s.cns_olfactory?.concentration_ppm??[0]),mode:s.flight_mode});
  }
  const stationary=[];
  for(let end=99;end<points.length;end+=5) {
    const window=points.slice(end-99,end+1);
    if(!window.every(p=>p.eligible))continue;
    const center=[0,1,2].map(axis=>mean(window.map(p=>p.p[axis])));
    const radius=Math.max(...window.map(p=>distance(p.p,center)));
    if(radius<=5)stationary.push({start_seconds:window[0].t,end_seconds:window.at(-1).t+0.1,
      radius_mm:radius,position_mm:center,mode:window.at(-1).mode});
  }
  // Locate repeated absolute paths across three cycles, not just repeated labels.
  const cycles=[];
  for(let period=30;period<=200;period+=5) {
    for(let start=0;start+3*period<=points.length;start+=5) {
      let squared=0,n=0,eligible=0,path=0;
      for(let j=0;j<period;j+=5) {
        const a=points[start+j],b=points[start+period+j],c=points[start+2*period+j];
        squared+=distance(a.p,b.p)**2+distance(b.p,c.p)**2;n+=2;
        eligible+=Number(a.eligible)+Number(b.eligible)+Number(c.eligible);
        if(j>=5)path+=distance(a.p,points[start+j-5].p);
      }
      const rms=Math.sqrt(squared/Math.max(1,n));
      if(eligible/(n*1.5)>=0.8&&path>30&&rms<=5)cycles.push({start_seconds:points[start].t,
        end_seconds:points[start+3*period-1].t+0.1,period_seconds:period*0.1,rms_mm:rms,path_per_cycle_mm:path});
    }
  }
  const activity={};
  for(const [key,minimum] of [['flying',1],['walking',0.5],['resting',0.5],['clean',2],['feeding',0.2],['safety',0]]) {
    const all=bouts(points,key,minimum);
    const valid=all.filter(b=>key==='flying'?b.displacement_mm>5:key==='walking'?b.displacement_mm>1:true);
    activity[key]={total_seconds:points.filter(p=>p[key]).length*0.1,bouts:valid,
      longest_seconds:Math.max(0,...valid.map(b=>b.duration_seconds))};
  }
  const failures=[];
  if(Math.abs(report.summary.duration_seconds-300)>0.0021)failures.push('duration is not 300 seconds');
  for(const resource of ['sugar_drop','flower_nectar'])if(!meals.some(e=>e.resource===resource))failures.push(`missing actual meal: ${resource}`);
  for(const key of ['flying','walking','resting'])if(!activity[key].bouts.length)failures.push(`missing physical ${key} bout`);
  if(report.summary.food_search_recovery_failure_windows!==0)failures.push('internal no-progress recovery failed');
  if(report.summary.invalid_hunger_relief_windows!==0)failures.push('hunger relief without actual feeding');
  if(stationary.length)failures.push('unexplained 10-second inactivity');
  if(cycles.length)failures.push('repeated absolute path candidates require resolution');
  if((report.summary.grooming_events??[]).some(e=>e.completed&&!validGroom(e)))failures.push('invalid physical grooming received completion credit');
  if(report.brain.neurons!==166700||report.timebase.neural_timestep_seconds!==0.0001
    ||report.timebase.physics_timestep_seconds!==0.0002)failures.push('frozen CNS/timebase mismatch');
  return {passed:!failures.length,failures,runtime_sha256:report.runtime_sha256,
    initial_state_sha256:report.summary.initial_state_sha256,meals,grooming,activity,
    spatial_cells_5mm:new Set(points.map(p=>p.p.map(x=>Math.floor(x/5)).join(','))).size,
    stationary,cycles,requires_safety_review:activity.safety.bouts.filter(b=>b.duration_seconds>=5),
    invalid_grooming_events:(report.summary.grooming_events??[]).filter(e=>!validGroom(e))};
}

if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href) {
  const [input,output]=process.argv.slice(2);
  if(!output)throw Error('usage: indoor_long_gate.mjs INPUT OUTPUT');
  const bytes=await readFile(input);
  const result=analyzeReport(JSON.parse(input.endsWith('.gz')?gunzipSync(bytes):bytes));
  await writeFile(output,JSON.stringify(result,null,2)+'\n',{flag:'wx'});
  console.log(JSON.stringify({...result,activity:undefined,stationary:result.stationary.length,cycles:result.cycles.length},null,2));
  if(!result.passed)process.exitCode=1;
}
