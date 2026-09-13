"""V4 delivery: actual new actions, readable subjects, low music and no effects."""
import json,hashlib,subprocess,re,sys
from pathlib import Path
import numpy as np
R=Path('outputs/social-video-v4')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def run(args):return subprocess.check_output(args,stderr=subprocess.STDOUT)
def pcm(p):return np.frombuffer(run(['ffmpeg','-v','error','-i',str(p),'-f','f32le','-ac','1','-ar','24000','-']),dtype='<f4')
def rms(a):return float(20*np.log10(max(1e-9,np.sqrt(np.mean(a*a)))))
rows=json.loads((R/'narration.json').read_text());shots=json.loads((R/'shots.json').read_text())
assert next(r for r in rows if r['name']=='web')['text']=='这就是它住的网页。'
assert not re.search('MuJoCo|MaleCNS',''.join(r['text'] for r in rows),re.I)
frames=sum(round(r['duration']*30) for r in rows);assert 1800<=frames<=2700
flight=json.loads((R/'sources/flight.json').read_text());walk=json.loads((R/'sources/walk_b.json').read_text())
s=next(s for s in shots if s['name']=='takeoff');fs=[f['snapshot'] for f in flight['display_replay']['frames'] if s['start']<=f['snapshot']['time_seconds']<=s['start']+s['span']]
assert {'GROUNDED','TAKEOFF','CRUISE'}<={f['flight_mode'] for f in fs}
distance=float(np.linalg.norm(np.array(fs[0]['root_position'][:2])-[48,-12]));assert distance>50
s=next(s for s in shots if s['name']=='explore')
assert {f['snapshot']['flight_mode'] for f in walk['display_replay']['frames'] if s['start']<=f['snapshot']['time_seconds']<=s['start']+s['span']}=={'GROUNDED'}
for source in ['groom_a','groom_b']:
 data=json.loads((R/f'sources/{source}.json').read_text());assert any(e['completed'] for e in data['summary']['grooming_events'])
for source,shot in [('flight','meal'),('flower','flower')]:
 data=flight if source=='flight' else json.loads((R/f'sources/{source}.json').read_text());s=next(s for s in shots if s['name']==shot)
 assert any(e['start_seconds']<=s['start'] and s['start']+s['span']<=e['end_seconds'] for e in data['summary']['actual_feeding_events'])
music=pcm(R/'portrait/audio/original-score.wav');sfx=pcm(R/'portrait/audio/designed-sfx.wav');oldmusic=pcm('outputs/social-video-v3/portrait/audio/original-score.wav')
assert np.max(np.abs(sfx))==0
assert rms(music)<rms(oldmusic)-7
report={'voice':'zh-CN-XiaoyiNeural +2%','duration_seconds':frames/30,'no_sound_effects':True,
 'music':'New original sparse warm keyboard arrangement; no percussion. Not the V3 score.',
 'music_rms_dbfs':rms(music),'v3_music_rms_dbfs':rms(oldmusic),'takeoff_distance_from_sugar_mm':distance,
 'fresh_sources':{n:sha(R/f'sources/{n}.json') for n in ['flight','walk_b','groom_a','groom_b','flower']},
 'new_rendered_shots':sum(not s.get('reuse',False) for s in shots)+4,'outputs':[],
 'narration_sha256':sha(R/'narration.json'),
 'visual_changes':'Observer-only camera fill and exposure; actual recorded native poses. No simulation behavior changes.',
 'audio_review':'ASR and objective level checks; no direct listening capability in this environment.',
 'credits':json.loads(Path('outputs/social-video-v3/delivery-validation.json').read_text())['credits']}
if '--sources-only' in sys.argv:print(json.dumps(report,ensure_ascii=False,indent=2));raise SystemExit(0)
for layout,size in [('portrait',[1080,1920]),('landscape',[1920,1080])]:
 d=R/layout;p=d/f'cyber-fly-{layout}-xiaoyi.mp4'
 info=json.loads(run(['ffprobe','-v','error','-show_streams','-show_format','-of','json',str(p)]));v=next(s for s in info['streams'] if s['codec_type']=='video')
 assert [v['width'],v['height']]==size and v['r_frame_rate']=='30/1' and v['pix_fmt']=='yuv420p' and int(v['nb_frames'])==frames
 assert not run(['ffmpeg','-v','error','-i',str(p),'-f','null','-']).strip()
 loud=run(['ffmpeg','-hide_banner','-i',str(p),'-vn','-af','ebur128=peak=true','-f','null','-']).decode()
 lufs=float(re.findall(r'I:\s*([\-\d.]+) LUFS',loud)[-1]);peak=float(re.findall(r'Peak:\s*([\-\d.]+) dBFS',loud)[-1]);assert -20<lufs<-13 and peak<0
 timeline=json.loads((d/'timeline.json').read_text());mid=[round((r['start']+r['duration']*.5)*30) for r in timeline];sel='+'.join(f'eq(n,{n})' for n in mid)
 scale,tile=('216:384','6x5') if layout=='portrait' else ('384:216','5x6')
 run(['ffmpeg','-v','error','-y','-i',str(p),'-vf',f"select='{sel}',scale={scale},tile={tile}",'-frames:v','1',str(d/'contact-sheet.jpg')])
 title=next(r for r in timeline if r['name']=='title');brightness=[]
 for index,fraction in enumerate([.1,.5,.9]):
  t=title['start']+title['duration']*fraction;file=d/f'title-check-{index}.jpg'
  run(['ffmpeg','-v','error','-y','-ss',str(t),'-i',str(p),'-frames:v','1',str(file)])
  roi=np.frombuffer(run(['ffmpeg','-v','error','-i',str(file),'-vf','crop=iw*0.55:ih*0.35:iw*0.225:ih*0.4,scale=100:100,format=gray','-f','rawvideo','-']),np.uint8)
  brightness.append(float(roi.mean()))
 assert min(brightness)>75,brightness
 audio=run(['ffmpeg','-v','error','-i',str(p),'-map','0:a:0','-c','copy','-f','adts','-'])
 report['outputs'].append(dict(layout=layout,sha256=sha(p),frames=frames,lufs=lufs,peak_dbfs=peak,title_roi_brightness=brightness,audio_sha256=hashlib.sha256(audio).hexdigest()))
assert report['outputs'][0]['audio_sha256']==report['outputs'][1]['audio_sha256']
(R/'delivery-validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2));print(json.dumps(report,ensure_ascii=False,indent=2))
