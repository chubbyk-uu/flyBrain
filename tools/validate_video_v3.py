"""V3 media checks, visual review sheets and objective mix audibility checks."""
import json,hashlib,subprocess,re,wave
from pathlib import Path
import numpy as np
ROOT=Path('outputs/social-video-v3')
def sh(args):return subprocess.check_output(args,stderr=subprocess.STDOUT)
def digest(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def pcm(p,phone=False):
 args=['ffmpeg','-v','error','-i',str(p),'-vn']
 if phone:args+=['-af','highpass=f=300,lowpass=f=6000']
 return np.frombuffer(sh(args+['-f','f32le','-ar','24000','-ac','1','-']),dtype='<f4')
def rms(a):return float(20*np.log10(max(1e-9,np.sqrt(np.mean(a*a)))))
rows=json.loads((ROOT/'narration.json').read_text());expected=sum(round(r['duration']*30) for r in rows)
assert [r['duration'] for r in rows[:3]]==[.7,.7,.7]
assert next(r for r in rows if r['name']=='world')['duration']<=1.5
assert next(r for r in rows if r['name']=='eyes_full')['duration']==2
assert sum(r['duration'] for r in rows[:next(i for i,r in enumerate(rows) if r['name']=='network')])>40
assert not re.search('MaleCNS|MuJoCo|真的能看见',''.join(r['text'] for r in rows),re.I)
report={'outputs':[],'duration_seconds':expected/30,'voice':'zh-CN-XiaoxiaoNeural +2%',
 'audio_review':'Objective mix, phone-band mono and ASR checks; no direct human/assistant listening capability in this environment.',
 'visual_sources':'Existing V2 native behavioral recordings, reframed and retimed. Retinal insert retains original paired native capture.',
 'sfx':'Designed illustrative sounds, not biological microphone recordings.',
 'credits':json.loads(Path('outputs/social-video-v2/delivery-validation.json').read_text())['credits']}
capture=json.loads((ROOT/'web-capture/capture.json').read_text())
samples=capture['samples'];assert len(samples)==78
feed=sum(s['metrics']['latestSnapshot']['behavior_mode']=='Feed' for s in samples)
assert feed>len(samples)*.5,feed
assert all(s['metrics']['connected'] for s in samples)
assert samples[-1]['metrics']['latestSnapshot']['time_seconds']>samples[0]['metrics']['latestSnapshot']['time_seconds']
report['web_capture']={'sha256':digest(ROOT/'web-capture/live.mp4'),'feeding_frames':feed,'frames':len(samples),
 'description':'Actual native viewer screenshot sequence, dedicated live CUDA instance with near-food spawn and slowed recording clock. Display camera/exposure/UI labels adjusted; not a realtime performance demonstration.',
 'simulation_time_range':[samples[0]['metrics']['latestSnapshot']['time_seconds'],samples[-1]['metrics']['latestSnapshot']['time_seconds']]}
for layout,size in [('portrait',[1080,1920]),('landscape',[1920,1080])]:
 folder=ROOT/layout;video=folder/f'cyber-fly-{layout}-xiaoxiao.mp4'
 info=json.loads(sh(['ffprobe','-v','error','-show_streams','-show_format','-of','json',str(video)]))
 v=next(s for s in info['streams'] if s['codec_type']=='video')
 assert [v['width'],v['height']]==size and v['r_frame_rate']=='30/1' and v['pix_fmt']=='yuv420p'
 assert int(v['nb_frames'])==expected,(layout,v['nb_frames'],expected)
 assert 60<=float(info['format']['duration'])<=90
 assert not sh(['ffmpeg','-v','error','-i',str(video),'-f','null','-']).strip()
 log=sh(['ffmpeg','-hide_banner','-i',str(video),'-vn','-af','ebur128=peak=true','-f','null','-']).decode()
 loud=float(re.findall(r'I:\s*([\-\d.]+) LUFS',log)[-1]);peak=float(re.findall(r'Peak:\s*([\-\d.]+) dBFS',log)[-1])
 assert -19<loud<-12 and peak<0,(loud,peak)
 timeline=json.loads((folder/'timeline.json').read_text());frames=[round((r['start']+r['duration']*.5)*30) for r in timeline]
 select='+'.join(f'eq(n,{n})' for n in frames)
 scale,tile=('216:384','6x5') if layout=='portrait' else ('384:216','5x6')
 sh(['ffmpeg','-v','error','-y','-i',str(video),'-vf',f"select='{select}',scale={scale},tile={tile}",'-frames:v','1',str(folder/'contact-sheet.jpg')])
 audio=sh(['ffmpeg','-v','error','-i',str(video),'-map','0:a:0','-c','copy','-f','adts','-'])
 report['outputs'].append(dict(layout=layout,sha256=digest(video),frames=expected,lufs=loud,peak_dbfs=peak,audio_sha256=hashlib.sha256(audio).hexdigest()))
assert report['outputs'][0]['audio_sha256']==report['outputs'][1]['audio_sha256']
folder=ROOT/'portrait';timeline=json.loads((folder/'timeline.json').read_text())
mix=pcm(folder/'cyber-fly-portrait-xiaoxiao.mp4',True);music=pcm(folder/'audio/original-score.wav',True);sfx=pcm(folder/'audio/designed-sfx.wav',True)
voice=pcm(folder/'audio/narration.wav',True);checks=[]
for r in timeline:
 if r['name'] not in ['wing_pause','meal_pause','flower_pause','face_pause']:continue
 a,b=round(r['start']*24000),round((r['start']+r['duration'])*24000)
 values={name:rms(data[a:b]) for name,data in [('mix',mix),('music',music),('sfx',sfx),('voice',voice)]}
 assert values['voice']<-65,values
 assert values['mix']>-38 and values['sfx']>-42,values
 checks.append(dict(segment=r['name'],phone_band_mono_dbfs=values))
report['breathing_mix_checks']=checks
old=pcm('outputs/social-video-v2/portrait/audio/original-score.wav',True)
report['music_phone_band_rms_dbfs']={'v2':rms(old),'v3':rms(music)}
(ROOT/'delivery-validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2));print(json.dumps(report,ensure_ascii=False,indent=2))
