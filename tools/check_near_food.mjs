// Fixed, reproducible approach fixtures; no resource coordinates are fed to control.
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { spawn } from 'node:child_process';
const [output, ablation = 'connected'] = process.argv.slice(2);
if (!output || !['connected', 'motor-off'].includes(ablation)) throw Error('usage: check_near_food.mjs OUTPUT [connected|motor-off]');
await mkdir(output, { recursive: true });
const results = [];
for (const [name, position, yaw] of [
  ['side', [42, -18, 32.1], 90],
  ['oblique', [54, -17, 32.1], 180],
  ['tangent', [47, -19, 32.1], 0],
]) {
  const path = `${output}/${name}.json`;
  const args = ['cns-check', '--scene', 'indoor-v2', '--duration-seconds', '20',
    '--initial-position-mm', ...position.map(String), '--initial-yaw-deg', String(yaw),
    '--initial-hunger', '0.9', '--behavior-seed', '11', '--disable-resource', 'flower_nectar',
    '--record-display', '--output', path];
  if (ablation === 'motor-off') args.push('--disconnect-motor-outputs');
  const child = spawn('target/release/flybrain-world', args, { stdio: ['ignore', 'ignore', 'inherit'],
    env: { ...process.env, LD_LIBRARY_PATH: `${process.cwd()}/work/mujoco/lib` } });
  await new Promise((resolve, reject) => { child.on('error', reject); child.on('exit', code => code === 0 ? resolve() : reject(Error(`exit ${code}`))); });
  const report = JSON.parse(await readFile(path));
  const meal = report.summary.actual_feeding_events.find(e => e.resource === 'sugar_drop');
  const result = { name, initial_taste: report.samples[0].taste_active, meal_seconds: meal?.validated_at_seconds ?? null,
    runtime_sha256: report.runtime_sha256, initial_state_sha256: report.summary.initial_state_sha256 };
  results.push(result); console.log(JSON.stringify(result));
}
const passed = results.every(r => !r.initial_taste && (ablation === 'motor-off'
  ? r.meal_seconds === null : r.meal_seconds !== null && r.meal_seconds <= 20));
await writeFile(`${output}/results.json`, JSON.stringify({ ablation, passed, results }, null, 2) + '\n');
if (!passed) process.exitCode = 1;
