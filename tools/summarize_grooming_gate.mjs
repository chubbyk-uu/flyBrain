import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
const [connected,probe,motor,output]=process.argv.slice(2);
if(!output)throw Error('usage: summarize_grooming_gate.mjs CONNECTED PROBE_OFF MOTOR_OFF OUTPUT');
const groups=await Promise.all([connected,probe,motor].map(async dir=>JSON.parse(await readFile(`${dir}/results.json`))));
for(const group of groups)assert.deepEqual(group.map(r=>r.surface).sort(),['floor','table']);
const rows=['floor','table'].map(surface=>{
  const [positive,gateOff,motorOff]=groups.map(g=>g.find(r=>r.surface===surface));
  const failures=[];
  if(!positive.passed)failures.push('actual autonomous geometry/timing/support gate');
  if(gateOff.events.length||motorOff.events.length)failures.push('disconnected control still initiated a bout');
  if(new Set([positive,gateOff,motorOff].map(r=>r.initial_state_sha256)).size!==1)failures.push('unpaired initial state');
  if(new Set([positive,gateOff,motorOff].map(r=>r.runtime_sha256)).size!==1)failures.push('different runtime');
  if([positive,gateOff,motorOff].some(r=>r.invalid_hunger_relief_windows!==0))failures.push('hunger relief without actual feeding');
  return {surface,passed:!failures.length,failures,event:positive.event,
    initial_state_sha256:positive.initial_state_sha256,runtime_sha256:positive.runtime_sha256};
});
const report={passed:rows.every(r=>r.passed),kind:'short full-CNS autonomous action and paired causal gates; not final 300-second acceptance',rows};
await writeFile(output,JSON.stringify(report,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify(report,null,2));
if(!report.passed)process.exitCode=1;
