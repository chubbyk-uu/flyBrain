import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { gunzipSync } from 'node:zlib';
const directory=process.argv[2]??'outputs/indoor-v2/stage-4';
const reports=[];
for(const seed of [11,13,17,19,23]) {
  const path=`${directory}/speed-seed${seed}-v3.json`;
  const bytes=await readFile(path).catch(async error=>{
    if(error.code!=='ENOENT')throw error;
    return gunzipSync(await readFile(`${path}.gz`));
  });
  const run=JSON.parse(bytes);
  const s=run.summary;
  assert.equal(run.brain.neurons,166700);
  assert.ok(s.maximum_command_speed_mm_s<=100+1e-6);
  assert.ok(s.maximum_command_acceleration_mm_s2<=250+1e-6);
  assert.ok(s.cruise_speed_p95_mm_s<=120);
  // This stronger bound includes collision windows; none are excluded.
  assert.ok(s.maximum_speed_mm_s<=150);
  const near=[];
  let nearSince=null;
  for(const sample of run.samples) {
    const eligible=sample.odor_guidance.active && sample.foraging_mode==='APPROACH'
      && !sample.collision_reflex_active && sample.flight_mode==='CRUISE';
    nearSince=eligible?(nearSince??sample.time_seconds):null;
    if(nearSince!==null&&sample.time_seconds-nearSince>=0.5) {
      near.push(Math.hypot(...sample.flight_command_velocity_mm_s));
    }
  }
  assert.ok(near.length>10,'missing settled approach samples');
  assert.ok(Math.min(...near)>=15&&Math.max(...near)<=40);
  reports.push({seed,raw_report:path.split('/').at(-1),sha256:createHash('sha256').update(bytes).digest('hex'),
    command_max_mm_s:s.maximum_command_speed_mm_s,command_acceleration_max_mm_s2:s.maximum_command_acceleration_mm_s2,
    cruise_p95_mm_s:s.cruise_speed_p95_mm_s,all_window_peak_mm_s:s.maximum_speed_mm_s,
    settled_approach_command_range_mm_s:[Math.min(...near),Math.max(...near)],
    realtime_factor:s.duration_seconds/s.elapsed_seconds,feeding_seconds:s.feeding_seconds});
}
await writeFile(`${directory}/speed-gate.json`,JSON.stringify({passed:true,criteria:'all five predefined seeds; instantaneous control-window speeds; collision peaks included; approach settled >=0.5s',reports},null,2)+'\n');
console.log(JSON.stringify(reports,null,2));
