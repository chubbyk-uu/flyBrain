import {readFile,writeFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
import {gunzipSync} from 'node:zlib';

export function analyzePair(normal,pulse) {
  const failures=[];
  if(normal.runtime_sha256!==pulse.runtime_sha256)failures.push('runtime mismatch');
  if(normal.summary.initial_state_sha256!==pulse.summary.initial_state_sha256)failures.push('initial state mismatch');
  for(const key of ['assets','pack_arrays','parameters'])if(JSON.stringify(normal.initial_state?.[key])!==JSON.stringify(pulse.initial_state?.[key]))failures.push(`${key} mismatch`);
  if(normal.samples.length!==pulse.samples.length)failures.push('sample count mismatch');
  if([normal,pulse].some(r=>Math.abs(r.summary.duration_seconds-300)>0.0021))failures.push('paired duration is not 300 seconds');
  const interventions=pulse.sensory_interventions??[];
  if(interventions.length!==2||interventions.some((e,i)=>e.resource!=='sugar_drop'||e.odor_enabled!==(i===1)||Math.abs(e.time_seconds-[2,12][i])>0.0021))failures.push('incorrect odor-only intervention');
  if((normal.sensory_interventions??[]).length)failures.push('normal run was perturbed');
  let squared=0,count=0,firstReadout=null,firstSpike=null,firstPosition=null,preMismatch=null;
  for(let i=0;i<normal.samples.length&&i<pulse.samples.length;i++) {
    const a=normal.samples[i],b=pulse.samples[i],t=a.time_seconds;
    if(Math.abs(t-b.time_seconds)>1e-6){failures.push('sample time mismatch');break;}
    const d2=a.root_position.reduce((sum,x,j)=>sum+(x-b.root_position[j])**2,0);
    const readout=JSON.stringify(a.cns_olfactory.band_rate_hz)!==JSON.stringify(b.cns_olfactory.band_rate_hz);
    const spikes=a.cns_olfactory.spike_delta!==b.cns_olfactory.spike_delta;
    if(t<2-1e-6&&(d2>0||readout||spikes))preMismatch??=t;
    if(t>=2-1e-6&&t<=20+1e-6){
      squared+=d2;count++;
      if(readout)firstReadout??=t;
      if(spikes)firstSpike??=t;
      if(d2>1e-12)firstPosition??=t;
    }
  }
  const rms=Math.sqrt(squared/Math.max(1,count));
  if(preMismatch!==null)failures.push('difference before intervention');
  if(!count||rms<=5)failures.push('2-20 second trajectory RMS not above 5 mm');
  if(firstReadout===null||firstSpike===null)failures.push('no measured CNS olfactory response');
  return {passed:!failures.length,failures,runtime_sha256:normal.runtime_sha256,
    initial_state_sha256:normal.summary.initial_state_sha256,interventions,
    samples_compared:count,trajectory_rms_mm:rms,first_pre_intervention_mismatch_seconds:preMismatch,
    first_olfactory_readout_difference_seconds:firstReadout,first_olfactory_spike_difference_seconds:firstSpike,
    first_position_difference_seconds:firstPosition};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){
  const [a,b,output]=process.argv.slice(2);
  if(!output)throw Error('usage: indoor_odor_pair_gate.mjs NORMAL PULSE OUTPUT');
  const load=async path=>{const bytes=await readFile(path);return JSON.parse(path.endsWith('.gz')?gunzipSync(bytes):bytes);};
  const result=analyzePair(await load(a),await load(b));
  await writeFile(output,JSON.stringify(result,null,2)+'\n',{flag:'wx'});
  console.log(JSON.stringify(result,null,2));
  if(!result.passed)process.exitCode=1;
}
