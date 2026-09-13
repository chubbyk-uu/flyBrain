// Preserve the complete numerical trace; the large display replay stays local.
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {gzipSync} from 'node:zlib';
import {dirname} from 'node:path';
const [input,output]=process.argv.slice(2);
if(!output)throw Error('usage: archive_native_evidence.mjs RAW_JSON OUTPUT_JSON_GZ');
const source=await readFile(input),report=JSON.parse(source);
const displayFrames=report.display_replay?.frames.length??0;
delete report.display_replay;
const archived=gzipSync(JSON.stringify(report),{level:9});
await mkdir(dirname(output),{recursive:true});
await writeFile(output,archived,{flag:'wx'});
const hash=b=>createHash('sha256').update(b).digest('hex');
const manifest={source:input,source_sha256:hash(source),archive:output,archive_sha256:hash(archived),
  runtime_sha256:report.runtime_sha256,samples:report.samples.length,display_frames_not_in_archive:displayFrames,
  omitted_fields:displayFrames?['display_replay']:[],note:'All numerical samples, initial state, interventions and physical events retained. Full native pose replay remains at source path.'};
await writeFile(`${output}.manifest.json`,JSON.stringify(manifest,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify(manifest));
