"""Delivery gates for the two edits and the synchronous native inserts."""
import hashlib,json,re,subprocess
from pathlib import Path
ROOT=Path('outputs/social-video-v2')
def digest(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
lines=json.loads((ROOT/'narration.json').read_text());shots=json.loads((ROOT/'shots.json').read_text())
assert '仿佛' not in next(r['text'] for r in lines if r['name']=='alive')
assert '你会觉得' in next(r['text'] for r in lines if r['name']=='alive')
assert not re.search('MaleCNS|MuJoCo', ''.join(r['text'] for r in lines),re.I)
studio=Path('web/film-studio-v2.js').read_text()
assert all(s not in studio for s in ['一个人的数字小世界','真实仿真轨迹','镜头回放'])
stereo=ROOT/'sources/stereo-telemetry'
take=json.loads((stereo/'take.json').read_text());frames=take['display_replay']['frames']
assert len(frames)>450
assert take['display_replay']['scene']['brain']['neurons']==166700
assert 'RTX 5080' in take['display_replay']['scene']['brain']['backend']
for f in frames:
 assert f['left_time']==f['right_time']==f['snapshot']['time_seconds']
 assert (stereo/f['retina_file']).stat().st_size==450*256
 assert f['snapshot']['filtered_population_rate_hz']>0
 assert f['snapshot']['cumulative_spiking_neuron_count']>0
assert len({f['snapshot']['filtered_population_rate_hz'] for f in frames})>10
assert len({digest(stereo/f['retina_file']) for f in frames})>50
journey=json.loads(Path('outputs/social-video-20260913/sources/journey.json').read_text())
def modes(name):
 s=next(s for s in shots if s['name']==name)
 return {f['snapshot']['flight_mode'].lower() for f in journey['display_replay']['frames'] if s['start']<=f['snapshot']['time_seconds']<=s['start']+s['span']}
assert modes('walk')=={'grounded'}
assert {'takeoff','cruise'}<=modes('takeoff')
assert {'landing','grounded'}<=modes('flight')
result={'voice':'zh-CN-XiaoxiaoNeural +2%', 'stereo_captures':len(frames),'maximum_stereo_body_clock_skew_seconds':0,
 'stereo_capture_binary_sha256':digest('target/release/examples/film_stereo_capture'),
 'stereo_source_sha256':digest(stereo/'take.json'),
 'capture_mode':'Both native eyes rendered synchronously at the same physical state as the body and neural snapshot; independent film-only processors. Not live asynchronous preview packets.',
 'physics_or_neural_behavior_changed':False,'narration_sha256':digest(ROOT/'narration.json'),
 'shot_manifest_sha256':digest(ROOT/'shots.json'),'outputs':[]}
previous=json.loads(Path('outputs/social-video-20260913/production-manifest.json').read_text())
result.update(credits=previous['credits'],behavior_sources=previous['sources'],
 music='Original deterministic synthesized score and sound effects; tools/finish_social_video.py',
 rendering='Offline display of recorded native body poses, with existing display-only wing carrier; independently framed portrait and landscape edits.',
 montage='Multiple reproducible initial-condition takes, not one continuous autonomous episode.',
 voice_validation='Independent ASR plus script-derived word-timed subtitles; ASR is not a guarantee of flawless pronunciation.',
 discarded_capture='sources/stereo: missing neural telemetry; replaced by sources/stereo-telemetry in all three data inserts.')
for layout,w,h in [('portrait',1080,1920),('landscape',1920,1080)]:
 folder=ROOT/layout;video=folder/f'cyber-fly-{layout}-xiaoxiao.mp4'
 probe=json.loads(subprocess.check_output(['ffprobe','-v','error','-show_streams','-show_format','-of','json',str(video)]))
 v=next(s for s in probe['streams'] if s['codec_type']=='video');a=next(s for s in probe['streams'] if s['codec_type']=='audio')
 assert (v['width'],v['height'],v['pix_fmt'],v['r_frame_rate'],v['codec_name'])==(w,h,'yuv420p','30/1','h264')
 assert a['codec_name']=='aac' and a['sample_rate']=='48000'
 assert 60<=float(probe['format']['duration'])<=90
 assert int(v['nb_frames'])==sum(round(r['duration']*30) for r in lines)
 decoded=subprocess.run(['ffmpeg','-v','error','-i',str(video),'-f','null','-'],capture_output=True,text=True,check=True);assert not decoded.stderr.strip(),decoded.stderr
 audio=subprocess.run(['ffmpeg','-hide_banner','-i',str(video),'-vn','-af','ebur128=peak=true','-f','null','-'],capture_output=True,text=True,check=True)
 (folder/'audio-loudness.txt').write_text(audio.stderr)
 peaks=re.findall(r'Peak:\s*([\-\d.]+) dBFS',audio.stderr);assert peaks and float(peaks[-1])<0
 loudness=re.findall(r'I:\s*([\-\d.]+) LUFS',audio.stderr);assert -20<float(loudness[-1])<-13
 timeline=json.loads((folder/'timeline.json').read_text());sample_frames=[round((r['start']+r['duration']*.5)*30) for r in timeline]
 select='+'.join(f'eq(n,{n})' for n in sample_frames)
 size,tile=('240:426','6x3') if layout=='portrait' else ('480:270','3x6')
 subprocess.run(['ffmpeg','-v','error','-y','-i',str(video),'-vf',f"select='{select}',scale={size},tile={tile}",'-frames:v','1',str(folder/'contact-sheet.jpg')],check=True)
 audio_bytes=subprocess.check_output(['ffmpeg','-v','error','-i',str(video),'-map','0:a:0','-c','copy','-f','adts','-'])
 result['outputs'].append(dict(layout=layout,file=str(video),sha256=digest(video),duration=float(probe['format']['duration']),frames=v['nb_frames'],size=[w,h],audio_sha256=hashlib.sha256(audio_bytes).hexdigest(),loudness_lufs=float(loudness[-1]),true_peak_dbfs=float(peaks[-1]),decode_errors=decoded.stderr))
assert result['outputs'][0]['audio_sha256']==result['outputs'][1]['audio_sha256'],'edits do not share identical narration/music'
(ROOT/'delivery-validation.json').write_text(json.dumps(result,ensure_ascii=False,indent=2));print(json.dumps(result,ensure_ascii=False,indent=2),flush=True)
