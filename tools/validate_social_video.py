import hashlib,json,re,subprocess
from pathlib import Path
out=Path('outputs/social-video-20260913');video=out/'cyber-fly-final-1080x1920.mp4'
probe=json.loads(subprocess.check_output(['ffprobe','-v','error','-show_streams','-show_format','-of','json',str(video)]))
v=next(s for s in probe['streams'] if s['codec_type']=='video');a=next(s for s in probe['streams'] if s['codec_type']=='audio')
assert (v['width'],v['height'],v['codec_name'],v['pix_fmt'],v['r_frame_rate'])==(1080,1920,'h264','yuv420p','30/1')
assert 60<=float(probe['format']['duration'])<=90 and a['codec_name']=='aac'
for name in ['shots.json','narration.json','subtitles.srt']:
 assert not re.search(r'MaleCNS|MuJoCo', (out/name).read_text(),re.I)
decode=subprocess.run(['ffmpeg','-v','error','-i',str(video),'-f','null','-'],capture_output=True,text=True,check=True)
assert not decode.stderr.strip(),decode.stderr
audio=subprocess.run(['ffmpeg','-hide_banner','-i',str(video),'-vn','-af','ebur128=peak=true','-f','null','-'],capture_output=True,text=True,check=True)
(out/'audio-loudness.txt').write_text(audio.stderr)
subprocess.run(['ffmpeg','-v','error','-y','-i',str(video),'-vf','fps=1/5,scale=270:480,tile=5x3','-frames:v','1',str(out/'final-contact-sheet.jpg')],check=True)
manifest=dict(output=video.name,sha256=hashlib.sha256(video.read_bytes()).hexdigest(),duration=float(probe['format']['duration']),width=1080,height=1920,fps=30,voice='zh-CN-YunxiNeural (+4%)',music='Original deterministic synthesized electronic score, tools/finish_social_video.py',sfx='Original synthesized effects; not biological recordings',subtitles='46 cues, original Chinese script with TTS word timestamps',decode_errors=decode.stderr,simulation_logic_modified=False,rendering='Offline Three.js rendering of actual native poses; quaternion interpolation; existing display-only wing carrier retained',montage='Four reproducible seed-11 initial-condition fixtures; food/grooming take selection, not one continuous autonomous episode',sensory_insert='Recorded native left/right retinal preview; separate from background replay take',neural_insert='Real recorded population aggregate telemetry; cumulative count is not concurrent active count; automatic labeled y-axis',asr='Independent small-model ASR and three male-voice auditions; ASR contains homophone spelling errors and does not certify flawless pronunciation',credits=[{'component':'FlyBrain','url':'https://github.com/mehrantsi/flyBrain','license':'MIT code; mixed-license assets; see THIRD_PARTY_NOTICES.md'},{'component':'MaleCNS v1.0 connectivity, transformed by project pack','credit':'FlyEM/HHMI Janelia, Cambridge, MRC LMB, Google Research and collaboration','url':'https://male-cns.janelia.org/download/','license':'CC BY 4.0'},{'component':'FlyGym / NeuroMechFly body and retina; FlyBody wing lineage','credit':'Respective research teams; project-adapted geometry and presentation','notice':'THIRD_PARTY_NOTICES.md and REFERENCES.md'}])
manifest['sources']={}
for name in ['sugar','flower','groom','journey']:
 p=out/'sources'/f'{name}.json';r=json.loads(p.read_text());manifest['sources'][name]=dict(sha256=hashlib.sha256(p.read_bytes()).hexdigest(),runtime_sha256=r['runtime_sha256'],initial_state_sha256=r['summary']['initial_state_sha256'],seed=r['initial_state']['behavior_seed'])
(out/'production-manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2))
print(json.dumps({k:manifest[k] for k in ['output','sha256','duration','width','height','fps','decode_errors']},indent=2))
