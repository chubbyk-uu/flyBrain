"""V4 production: fresh takes, Xiaoyi narration, quiet new score, no SFX."""
import asyncio,json,subprocess,sys
from pathlib import Path
import video_v3 as base
ROOT=Path('outputs/social-video-v4')
def voices():
 base.OUT=ROOT;base.VOICE='zh-CN-XiaoyiNeural'
 base.PLAN=[(n,'这就是它住的网页。' if n=='web' else t,c,d,p) for n,t,c,d,p in base.PLAN]
 asyncio.run(base.voices())
def capture():
 (ROOT/'sources').mkdir(parents=True,exist_ok=True)
 cases=[('flight',90,[-20,-30,32.1],15,.30,.05,21),
 ('groom_a',8,[-38,24,32.1],-35,.25,.95,22),
 ('groom_b',8,[12,20,32.1],65,.25,.95,23),
 ('flower',16,[-70,-4,32.1],30,.80,.1,24),
 ('walk',8,[20,-5,32.1],120,.85,.1,25),
 ('walk_b',5,[26,-18,32.1],0,.72,.1,11)]
 for name,dur,pos,yaw,hunger,urge,seed in cases:
  out=ROOT/'sources'/f'{name}.json'
  if out.exists():continue
  args=['target/release/flybrain-world','cns-check','--scene','indoor-v2','--duration-seconds',str(dur),'--initial-position-mm',*map(str,pos),f'--initial-yaw-deg={yaw}','--initial-hunger',str(hunger),'--initial-dirt',str(urge),'--behavior-seed',str(seed),'--record-display','--output',str(out)]
  print('START',name,flush=True)
  with (ROOT/'sources'/f'{name}.log').open('w') as log:subprocess.run(args,stdout=log,stderr=log,check=True)
  print('DONE',name,flush=True)
def plan():
 old='outputs/social-video-20260913/sources/'
 def shot(name,source,start,span,view='side',distance=10,height=4,angle=None):
  d=dict(name=name,source=source,input=str(ROOT/'sources'/f'{source}.json'),start=start,span=span,view=view,distance=distance,height=height,filmLighting=True)
  if source.startswith('old_'):d['input']=old+source[4:]+'.json'
  if angle is not None:d['angle']=angle
  return d
 shots=[
  shot('flash_fly','flight',18,.7,distance=10),
  shot('flash_meal','old_sugar',2.5,.7,distance=7,height=2.5,angle=.6),
  shot('flash_face','groom_b',2.8,.7,'front',7,3),
  shot('title','flight',10.2,1.2,distance=10,height=3.5),
  shot('explore','walk_b',.1,2.8,distance=11,height=4,angle=.9),
  shot('world','groom_b',5,1.1,'world'),
  shot('takeoff','flight',.4,1.2,distance=15,height=5),
  shot('wing_pause','flight',11.6,.8,distance=9,height=3,angle=.8),
  shot('landing','flight',44.8,2.5,distance=15,height=5),
  shot('sugar','flight',72.95,1.25,distance=10,height=3.5),
  shot('meal','flight',74.3,1.15,distance=7,height=2.4),
  shot('meal_pause','flight',75.5,.4,distance=6.8,height=3,angle=-.75),
  shot('flower','flower',7.8,1.5,distance=9,height=4),
  shot('flower_pause','flower',9.35,.45,distance=8,height=3.5,angle=-.7),
  shot('groom','groom_a',.6,1.8,'front',7,2.8),
  shot('face','groom_a',2.45,1.5,'front',6.5,2.8),
  shot('face_pause','groom_a',3.98,.5,'front',6.5,2.8),
  *[dict(name=n,reuse=True) for n in ['eyes_intro','eyes_full','eyes_back','web']],
  shot('reveal','groom_b',5,2.5,distance=9,height=4,angle=.4),
  dict(name='network',reuse=True),
  shot('origin','flight',22,3,distance=9,height=3,angle=.7),
  shot('hybrid','flower',4,3.5,distance=11,height=4,angle=.5),
  shot('honest','groom_b',.6,1.8,'front',7,3,angle=.3),
  dict(name='emotion',reuse=True),
  shot('alive','flight',32,2.4,distance=10,height=4,angle=-.7),
  shot('outro','flight',34.4,.8,distance=12,height=4,angle=-.7)]
 (ROOT/'shots.json').write_text(json.dumps(shots,ensure_ascii=False,indent=2))
 rows=json.loads((ROOT/'narration.json').read_text());nf=round(next(r['duration'] for r in rows if r['name']=='emotion')*30)
 extras=[shot('emotion_0','flight',16.6,.9,distance=10,height=3),shot('emotion_1','walk_b',3,.8,distance=9,height=3.5),shot('emotion_2','flower',9.85,.25,distance=8,height=3.5),shot('emotion_3','groom_b',3.55,.85,'front',7,3)]
 for i,s in enumerate(extras):s['duration']=(nf//4+(i<nf%4))/30
 (ROOT/'extras.json').write_text(json.dumps(extras,ensure_ascii=False,indent=2))
def join_emotion(layout):
 d=ROOT/layout/'shots';listing=d/'emotion.concat'
 words=[json.loads(x) for x in (ROOT/'audio/emotion.words.jsonl').read_text().splitlines()]
 row=next(r for r in json.loads((ROOT/'narration.json').read_text()) if r['name']=='emotion')
 cuts=[0]+[round(next(w['offset'] for w in words if w['text']==term)/1e7*30) for term in ['停下','舔','擦']]+[round(row['duration']*30)]
 for i in range(4):
  n=cuts[i+1]-cuts[i];source=d/f'emotion_{i}.mp4';duration=base.probe(source)
  base.run(['ffmpeg','-v','error','-y','-i',str(source),'-vf',f'setpts=(PTS-STARTPTS)*{n/30/duration},fps=30,tpad=stop_mode=clone:stop_duration=0.1','-frames:v',str(n),'-an','-c:v','libx264','-preset','fast','-crf','18','-pix_fmt','yuv420p',str(d/f'emotion_{i}-timed.mp4')])
 listing.write_text(''.join(f"file 'emotion_{i}-timed.mp4'\n" for i in range(4)))
 base.run(['ffmpeg','-v','error','-y','-f','concat','-safe','0','-i',str(listing),'-c','copy',str(d/'emotion.mp4')])
def reuse(layout):
 rows=json.loads((ROOT/'narration.json').read_text());shots=json.loads((ROOT/'shots.json').read_text())
 dest=ROOT/layout/'shots';dest.mkdir(parents=True,exist_ok=True)
 for s in shots:
  if not s.get('reuse'):continue
  name=s['name'];duration=next(r['duration'] for r in rows if r['name']==name)
  source=Path('outputs/social-video-v3')/layout/'shots'/f'{name}.mp4';old=base.probe(source)
  vf=f'setpts=(PTS-STARTPTS)*{duration/old},fps=30,tpad=stop_mode=clone:stop_duration=0.1'
  base.run(['ffmpeg','-v','error','-y','-i',str(source),'-vf',vf,'-frames:v',str(round(duration*30)),'-an','-c:v','libx264','-preset','fast','-crf','18','-pix_fmt','yuv420p',str(dest/f'{name}.mp4')])
if __name__=='__main__':
 if sys.argv[1]=='voice':voices()
 elif sys.argv[1]=='capture':capture()
 elif sys.argv[1]=='plan':plan()
 elif sys.argv[1]=='join':join_emotion(sys.argv[2])
 else:reuse(sys.argv[1])
