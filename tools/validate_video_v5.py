"""Validate the V5 voice/text revision, retaining V4 visual provenance."""
import hashlib,json,re,subprocess
from pathlib import Path
import numpy as np
from video_v5 import ROOT,ENDING
def run(args):return subprocess.check_output(args,stderr=subprocess.STDOUT)
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
rows=json.loads((ROOT/'narration.json').read_text())
assert ''.join(r['text'] for r in rows if r['name'] in ['honest','emotion','alive'])==ENDING
assert json.loads((ROOT/'voice-config.json').read_text())['voice']=='zh-CN-XiaoxiaoNeural'
frames=sum(round(r['duration']*30) for r in rows)
assert 1800<=frames<=2700
report={'duration_seconds':frames/30,'ending':ENDING,'voice':'zh-CN-XiaoxiaoNeural +2%',
 'visual_provenance':'Retimed existing V4 footage; no simulation or behavior changes.',
 'v4_validation_sha256':sha('outputs/social-video-v4/delivery-validation.json'),
 'audio_review':'ASR and objective levels; no direct listening capability.', 'outputs':[]}
for layout,size in [('portrait',[1080,1920]),('landscape',[1920,1080])]:
 d=ROOT/layout;p=d/f'cyber-fly-{layout}-xiaoxiao.mp4'
 info=json.loads(run(['ffprobe','-v','error','-show_streams','-show_format','-of','json',str(p)]))
 v=next(s for s in info['streams'] if s['codec_type']=='video')
 assert [v['width'],v['height']]==size and v['r_frame_rate']=='30/1' and int(v['nb_frames'])==frames
 assert not run(['ffmpeg','-v','error','-i',str(p),'-f','null','-']).strip()
 loud=run(['ffmpeg','-hide_banner','-i',str(p),'-vn','-af','ebur128=peak=true','-f','null','-']).decode()
 lufs=float(re.findall(r'I:\s*([\-\d.]+) LUFS',loud)[-1]);peak=float(re.findall(r'Peak:\s*([\-\d.]+) dBFS',loud)[-1])
 assert -20<lufs<-13 and peak<0
 levels={}
 for name in ['original-score','designed-sfx']:
  pcm=np.frombuffer(run(['ffmpeg','-v','error','-i',str(d/f'audio/{name}.wav'),'-f','f32le','-ac','1','-']),dtype='<f4')
  levels[name]=float(20*np.log10(max(1e-9,np.sqrt(np.mean(pcm*pcm)))))
  if name=='designed-sfx':assert np.max(np.abs(pcm))==0
 assert levels['original-score']<-31
 timeline=json.loads((d/'timeline.json').read_text())
 sel='+'.join(f"eq(n,{round((r['start']+r['duration']*.5)*30)})" for r in timeline)
 scale,tile=('216:384','6x5') if layout=='portrait' else ('384:216','5x6')
 run(['ffmpeg','-v','error','-y','-i',str(p),'-vf',f"select='{sel}',scale={scale},tile={tile}",'-frames:v','1',str(d/'contact-sheet.jpg')])
 audio=run(['ffmpeg','-v','error','-i',str(p),'-map','0:a:0','-c','copy','-f','adts','-'])
 report['outputs'].append(dict(layout=layout,sha256=sha(p),frames=frames,lufs=lufs,peak_dbfs=peak,levels=levels,audio_sha256=hashlib.sha256(audio).hexdigest()))
assert report['outputs'][0]['audio_sha256']==report['outputs'][1]['audio_sha256']
(ROOT/'delivery-validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2))
print(json.dumps(report,ensure_ascii=False,indent=2))
