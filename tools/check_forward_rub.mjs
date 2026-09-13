// Geometry check on actual full-CNS/MuJoCo body poses, not joint targets.
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
const [input,output]=process.argv.slice(2);
if(!output)throw Error('usage: check_forward_rub.mjs REPORT OUTPUT');
const r=JSON.parse(await readFile(input));
const index=name=>{const i=r.display_replay.scene.bodies.findIndex(b=>b.name===name);assert(i>=0,name);return i*7;};
const root=index('fly/c_thorax'),head=index('fly/c_head'),left=index('fly/lf_tarsus5'),right=index('fly/rf_tarsus5');
const rows=r.display_replay.frames.filter(f=>f.snapshot.grooming_active&&f.snapshot.grooming_phase>=0.2625&&f.snapshot.grooming_phase<=0.6375).map(f=>{
  const p=f.poses,[w,x,y,z]=p.slice(root+3,root+7);
  let fx=1-2*(y*y+z*z),fy=2*(x*y+w*z);const norm=Math.hypot(fx,fy);fx/=norm;fy/=norm;
  const ahead=i=>(p[i]-p[head])*fx+(p[i+1]-p[head+1])*fy;
  return {t:f.snapshot.time_seconds,forward:Math.min(ahead(left),ahead(right)),
    lateral:Math.abs((p[left]-p[right])*(-fy)+(p[left+1]-p[right+1])*fx),
    distance:Math.hypot(...[0,1,2].map(i=>p[left+i]-p[right+i])),
    slide:(p[left]-p[right])*fx+(p[left+1]-p[right+1])*fy};
});
assert(rows.length>=70,'at least 1.4 seconds of recorded paired rub');
const minForward=Math.min(...rows.map(r=>r.forward)),maxLateral=Math.max(...rows.map(r=>r.lateral));
const maxDistance=Math.max(...rows.map(r=>r.distance));
const slideRange=Math.max(...rows.map(r=>r.slide))-Math.min(...rows.map(r=>r.slide));
assert(minForward>0.45,'feet must extend beyond head');
assert(maxLateral<0.15,'feet must stay together laterally');
assert(maxDistance<0.35,'tip separation during rubbing');
assert(slideRange>0.15,'actual reciprocal sliding, not a frozen pose');
assert(r.summary.grooming_events.some(e=>e.completed&&e.minimum_support_legs>=4),'supported completion');
const report={passed:true,input,runtime_sha256:r.runtime_sha256,frames:rows.length,
  duration_seconds:rows.at(-1).t-rows[0].t,minimum_forward_mm:minForward,
  maximum_lateral_gap_mm:maxLateral,maximum_tip_distance_mm:maxDistance,relative_slide_range_mm:slideRange};
await writeFile(output,JSON.stringify(report,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify(report));
