"""Reproducible edit, original synthesized score/SFX, timed Chinese subtitles."""
import json,math,re,subprocess,wave,os
from pathlib import Path
import numpy as np
BASE=Path(os.environ.get('FLYBRAIN_VIDEO_OUT','outputs/social-video-20260913'))
LAYOUT=os.environ.get('FLYBRAIN_VIDEO_LAYOUT','')
V3=BASE.name=='social-video-v3'
V4=BASE.name=='social-video-v4'
V5=BASE.name=='social-video-v5'
assert LAYOUT in ['', 'portrait','landscape']
OUT=BASE/LAYOUT if LAYOUT else BASE
(OUT/'audio').mkdir(parents=True,exist_ok=True)
WIDTH,HEIGHT=(1920,1080) if LAYOUT=='landscape' else (1080,1920)
SR=48000
lines=json.loads((BASE/'narration.json').read_text());cursor=0
for row in lines:
 row['start']=cursor;row['duration']=math.ceil(row['duration']*30-1e-7)/30;cursor+=row['duration']
duration=cursor;count=round(duration*SR)
voice=np.zeros(count,np.float32);music=np.zeros((count,2),np.float32);sfx=np.zeros((count,2),np.float32)
def save(path,data):
 with wave.open(str(path),'wb') as w:
  w.setnchannels(1 if data.ndim==1 else data.shape[1]);w.setsampwidth(2);w.setframerate(SR);w.writeframes((np.clip(data,-1,1)*32767).astype('<i2').tobytes())
for row in lines:
 pcm=np.frombuffer(subprocess.check_output(['ffmpeg','-v','error','-i',row['audio'],'-f','f32le','-ac','1','-ar',str(SR),'-']),dtype='<f4')
 start=round(row['start']*SR);n=min(len(pcm),count-start);voice[start:start+n]=pcm[:n]
save(OUT/'audio'/'narration.wav',voice)
# Original restrained electronic miniature: warm pads, pentatonic plucks and soft beat.
rng=np.random.default_rng(20260913);beat=60/96
def add(dst,at,sound,pan=0):
 begin=round(at*SR);n=min(len(sound),len(dst)-begin)
 if n<=0:return
 dst[begin:begin+n,0]+=sound[:n]*math.sqrt((1-pan)/2);dst[begin:begin+n,1]+=sound[:n]*math.sqrt((1+pan)/2)
def tone(freq,length):
 t=np.arange(round(length*SR))/SR
 return t,np.sin(2*np.pi*freq*t)
chords=[[130.81,164.81,196],[110,130.81,164.81],[87.31,130.81,174.61],[98,146.83,196]]
for bar,at in enumerate(np.arange(0,duration,beat*8)):
 t=np.arange(round(beat*8*SR))/SR;env=np.minimum(t/.7,1)*np.minimum((beat*8-t)/.8,1)
 chord=sum(np.sin(2*np.pi*f*t+.08*np.sin(2*np.pi*.2*t)) for f in chords[bar%4])/3
 add(music,float(at),.047*chord*env,-.15)
notes=[523.25,659.25,783.99,659.25,440,523.25,659.25,587.33]
for k,at in enumerate(np.arange(0,duration,beat)):
 t,s=tone(notes[k%8],.8);pluck=.09*(s+.2*np.sin(2*np.pi*notes[k%8]*2*t))*np.exp(-t*7)*np.minimum(t/.006,1)
 if k%2==0:add(music,float(at),pluck,.4 if k%4==0 else -.4)
 if k%4==0:
  t=np.arange(round(.2*SR))/SR;kick=.10*np.sin(2*np.pi*(48*t+5*(1-np.exp(-t*22))))*np.exp(-t*22);add(music,float(at),kick)
 if k%2==1:
  t=np.arange(round(.07*SR))/SR;noise=rng.normal(0,1,len(t));noise=np.diff(noise,prepend=0);add(music,float(at),.013*noise*np.exp(-t*65),.2)
for row in lines:
 if row['name'] in ['takeoff','retina','neural']:
  t=np.arange(round(.5*SR))/SR;sw=rng.normal(0,1,len(t));sw=np.convolve(sw,np.ones(12)/12,'same')
  add(sfx,row['start'],.045*sw*np.sin(np.pi*t/.5)**2,.1)
 if row['name']=='takeoff':
  t=np.arange(round(2.5*SR))/SR;buzz=.013*(np.sin(2*np.pi*218*t)+.3*np.sin(2*np.pi*436*t))*np.minimum(t/.2,1)*np.minimum((2.5-t)/.4,1)
  add(sfx,row['start']+.5,buzz,-.15)
 if row['name'] in ['meal','flower']:
  t,s=tone(1046.5,.28);add(sfx,row['start']+.12,.045*s*np.exp(-t*20),.25)
fade=np.minimum(np.arange(count)/SR/1.5,1)*np.minimum((count-np.arange(count))/SR/2.3,1)
if V3:
 # Audible midrange effects, designed for storytelling rather than biological recordings.
 music*=3.2
 for row in lines:
  name=row['name'];at=row['start'];length=row['duration']
  if name in ['flash_fly','takeoff','wing_pause','outro']:
   t=np.arange(round(length*SR))/SR
   env=np.minimum(t/.06,1)*np.minimum((length-t)/.15,1)
   buzz=(np.sin(2*np.pi*218*t)+.45*np.sin(2*np.pi*436*t)+.25*np.sin(2*np.pi*872*t))
   add(sfx,at,.095*buzz*env,-.1)
  if name in ['flash_meal','meal_pause','flower_pause']:
   for dt in np.arange(.08,length,.24):
    t=np.arange(round(.075*SR))/SR
    drop=np.sin(2*np.pi*(850*t+55*(1-np.exp(-t*40))))*np.exp(-t*65)*np.minimum(t/.004,1)
    add(sfx,at+float(dt),.32*drop,.15)
  if name in ['flash_face','face_pause','groom','face']:
   for dt in np.arange(.06,length,.32):
    t=np.arange(round(.16*SR))/SR;noise=rng.normal(0,1,len(t));noise=np.convolve(noise,np.ones(8)/8,'same')
    rub=noise*np.sin(np.pi*t/.16)**2
    add(sfx,at+float(dt),.27*rub,-.2)
  if name=='landing':
   t=np.arange(round(.12*SR))/SR
   add(sfx,at+length-.35,.08*np.sin(2*np.pi*360*t)*np.exp(-t*45))
if V4 or V5:
 # Entirely new, sparse warm keyboard arrangement. No percussion or foley.
 music[:]=0;sfx[:]=0
 phrases=[[261.63,329.63,392.00,493.88],[220,261.63,329.63,392],[174.61,220,261.63,329.63],[196,246.94,293.66,392]]
 for k,at in enumerate(np.arange(0,duration,1.45)):
  f=phrases[(k//4)%4][k%4];t=np.arange(round(3.6*SR))/SR
  env=(1-np.exp(-t*24))*np.exp(-t*1.3)*np.minimum((3.6-t)/.2,1)
  note=(np.sin(2*np.pi*f*t)+.20*np.sin(2*np.pi*f*2*t)*np.exp(-t*2)+.06*np.sin(2*np.pi*f*3*t)*np.exp(-t*4))
  add(music,float(at),note*env*.03,(-.15,.12,.05,-.1)[k%4])
 # Fixed low-level master; approximately 10 dB quieter than the V3 bed.
 level=float(np.sqrt(np.mean(music**2)));music*=10**(-35/20)/max(level,1e-9)
save(OUT/'audio'/'original-score.wav',music*fade[:,None]);save(OUT/'audio'/'designed-sfx.wav',sfx)
def timestamp(t,ass=False):
 h=int(t//3600);m=int(t//60)%60;s=t%60
 return f'{h}:{m:02}:{s:05.2f}' if ass else f'{h:02}:{m:02}:{int(s):02},{round((s-int(s))*1000):03}'
subs=[]
for row in lines:
 words=[json.loads(x) for x in (BASE/'audio'/f"{row['name']}.words.jsonl").read_text().splitlines()]
 chunks=re.findall(r'[^，。？！、…]+[，。？！、…]*',row['text']);wi=0
 for chunk in chunks:
  clean=re.sub(r'[，。？！、…]','',chunk);gather=[];n=0
  while wi<len(words) and n<len(clean):
   w=words[wi];wi+=1;gather.append(w);n+=len(re.sub(r'\W','',w['text'],flags=re.UNICODE))
  if gather:
   start=row['start']+gather[0]['offset']/1e7;end=row['start']+(gather[-1]['offset']+gather[-1]['duration'])/1e7+.12
   display=clean
   if LAYOUT=='portrait' and len(clean)>18:display=clean[:len(clean)//2]+'\n'+clean[len(clean)//2:]
   subs.append((start,min(end,row['start']+row['duration']),display))
header='''[Script Info]
ScriptType: v4.00+
PlayResX: 1080
PlayResY: 1920
WrapStyle: 0
[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Microsoft YaHei,48,&H00FFFFFF,&H00FFFFFF,&H0020160B,&H99000000,-1,0,0,0,100,100,1,0,1,3,1,2,80,80,172,1
[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
'''
if LAYOUT:
 header=header.replace('PlayResX: 1080',f'PlayResX: {WIDTH}').replace('PlayResY: 1920',f'PlayResY: {HEIGHT}')
 header=header.replace('Microsoft YaHei,48,',f"Microsoft YaHei,{48 if LAYOUT=='landscape' else 52},")
 header=header.replace(',80,80,172,1',',100,100,85,1' if LAYOUT=='landscape' else ',90,90,230,1')
 if V3 or V4 or V5:header=header.replace(',100,100,85,1',',140,140,140,1').replace(',90,90,230,1',',110,130,350,1')
events=[]
for a,b,t in subs:
 t=t.replace('\n',r'\N')
 events.append(f'Dialogue: 0,{timestamp(a,True)},{timestamp(b,True)},Default,,0,0,0,,{{\\fad(60,60)}}{t}\n')
if V3 or V4 or V5:
 labels={'groom':'前脚搓一搓','face':'再擦擦眼睛','eyes_full':'它眼中的模拟世界','network':'16.7 万个神经元','web':'这就是它住的网页'}
 for row in lines:
  if row['name'] in labels:
   a=row['start'];b=a+min(row['duration'],2.0);label=labels[row['name']]
   x,y=(WIDTH//2,170 if LAYOUT=='landscape' else 210)
   events.append(f'Dialogue: 1,{timestamp(a,True)},{timestamp(b,True)},Default,,0,0,0,,{{\\an8\\pos({x},{y})\\fs58\\c&HDCF58F&\\fad(100,150)}}{label}\n')
(OUT/'subtitles.ass').write_text(header+''.join(events))
(OUT/'subtitles.srt').write_text(''.join(f'{i+1}\n{timestamp(a)} --> {timestamp(b)}\n{t}\n\n' for i,(a,b,t) in enumerate(subs)))
(OUT/'timeline.json').write_text(json.dumps(lines,ensure_ascii=False,indent=2))
(OUT/'shots.concat').write_text(''.join(f"file 'shots/{r['name']}.mp4'\n" for r in lines))
print('Prepared audio and subtitles:',duration,'seconds;',len(subs),'cues',flush=True)
if '--render' in __import__('sys').argv:
 vf=f"[0:v]ass={OUT}/subtitles.ass,fade=t=in:d=0.12,fade=t=out:st={duration-.6}:d=0.6,scale=in_range=pc:out_range=tv:out_color_matrix=bt709,format=yuv420p[v]"
 fade_audio=.35 if LAYOUT else .8
 af=f"[1:a]loudnorm=I=-16:TP=-2:LRA=8,asplit=2[voice][key];[2:a][key]sidechaincompress=threshold=0.025:ratio={2 if V3 else 3}:attack=15:release=250[bed];[voice][bed][3:a]amix=inputs=3:normalize=0,alimiter=limit=0.89:level={'false' if V3 else 'true'},afade=t=out:st={duration-fade_audio}:d={fade_audio}[a]"
 filename=f"cyber-fly-{LAYOUT}-{'xiaoyi' if V4 else 'xiaoxiao'}.mp4" if LAYOUT else 'cyber-fly-final-1080x1920.mp4'
 subprocess.run(['ffmpeg','-hide_banner','-loglevel','warning','-y','-f','concat','-safe','0','-i',str(OUT/'shots.concat'),'-i',str(OUT/'audio/narration.wav'),'-i',str(OUT/'audio/original-score.wav'),'-i',str(OUT/'audio/designed-sfx.wav'),'-filter_complex',vf+';'+af,'-map','[v]','-map','[a]','-c:v','libx264','-preset','medium','-crf','18','-pix_fmt','yuv420p','-color_range','tv','-colorspace','bt709','-color_primaries','bt709','-color_trc','bt709','-c:a','aac','-b:a','192k','-ar','48000','-t',str(duration),'-movflags','+faststart',str(OUT/filename)],check=True)
