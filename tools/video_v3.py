"""V3 story edit: retain native recorded behavior; revise only presentation/audio."""
import asyncio,json,os,math,subprocess,sys
from pathlib import Path
import edge_tts
OUT=Path('outputs/social-video-v3'); OLD=Path('outputs/social-video-v2')
VOICE='zh-CN-XiaoxiaoNeural'
# name, spoken text, underlying V2 clips, minimum duration, quiet tail
PLAN=[
 ('flash_fly','',['scan'],.7,0),
 ('flash_meal','',['meal'],.7,0),
 ('flash_face','',['groom'],.7,0),
 ('title','我在电脑里，养了一只赛博果蝇。',['hook'],3,.2),
 ('explore','先四处逛逛，看看今天有什么好吃的。',['walk','busy'],0,.25),
 ('world','',['hybrid'],1.1,0),
 ('takeoff','走着不过瘾，那就飞一段。',['takeoff'],0,.2),
 ('wing_pause','',['scan'],.8,0),
 ('landing','飞累了，找个地方落下来。',['flight'],0,.3),
 ('sugar','嗯，有甜味。',['sugar'],0,.35),
 ('meal','找到糖水了。看这张小嘴，真的在吃。',['meal'],0,.3),
 ('meal_pause','',['meal'],.8,0),
 ('flower','换个地方，花心里还有一餐。',['flower'],0,.45),
 ('flower_pause','',['flower'],.7,0),
 ('groom','再看看这两只前脚，先搓一搓。',['groom'],0,.2),
 ('face','然后，连眼睛也要擦一擦。',['alive'],0,.2),
 ('face_pause','',['alive'],.8,0),
 ('eyes_intro','对了，换到它的视角呢？',['retina'],0,.2),
 ('eyes_full','',['retina'],2,0),
 ('eyes_back','这是它左右复眼里的模拟世界。',['retina'],0,.2),
 ('web','',['web'],2.6,0),
 ('reveal','这些，并不是按剧本播放的动画。',['scan','walk'],0,.2),
 ('network','它背后，是十六万七千个神经元构成的网络。',['neural'],0,.2),
 ('origin','连接结构，来自科学家最近公布的真实果蝇神经连接图。',['scan','flower'],0,.2),
 ('hybrid','我给它接上物理身体，也加上饥饿、疲劳这些规则。',['walk','meal'],0,.2),
 ('honest','它当然还不是真正的生命。',['honest'],0,.3),
 ('emotion','但看着它自己飞起来，停下，舔糖，擦眼睛。',['takeoff','flight','meal','groom'],0,.2),
 ('alive','确实会有那么一瞬间，你会觉得，它好像活了。',['end'],0,.35),
 ('outro','',['end'],1,0),
]
def run(args):subprocess.run(args,check=True)
def probe(p):return float(subprocess.check_output(['ffprobe','-v','error','-show_entries','format=duration','-of','csv=p=0',str(p)]))
async def voices():
 (OUT/'audio').mkdir(parents=True,exist_ok=True);rows=[]
 for name,text,clips,minimum,tail in PLAN:
  raw=OUT/'audio'/f'{name}-raw.mp3';audio=OUT/'audio'/f'{name}.wav';meta=OUT/'audio'/f'{name}.words.jsonl'
  if text:
   if not meta.exists():await edge_tts.Communicate(text,VOICE,rate='+2%',boundary='WordBoundary',proxy=os.environ.get('HTTPS_PROXY')).save(str(raw),str(meta))
   words=[json.loads(x) for x in meta.read_text().splitlines()];end=max((w['offset']+w['duration'])/1e7 for w in words)
   duration=math.ceil(max(minimum,end+tail)*30)/30
   run(['ffmpeg','-v','error','-y','-i',str(raw),'-af','apad','-t',str(duration),'-ar','48000','-ac','1',str(audio)])
  else:
   duration=minimum;end=0;meta.write_text('')
   run(['ffmpeg','-v','error','-y','-f','lavfi','-i','anullsrc=r=48000:cl=mono','-t',str(duration),str(audio)])
  rows.append(dict(name=name,text=text,audio=str(audio),duration=duration,voice_duration=end,clips=clips))
  print(name,duration,flush=True)
 assert 60<=sum(r['duration'] for r in rows)<=90
 (OUT/'narration.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2));print('TOTAL',sum(r['duration'] for r in rows),flush=True)
def edit(layout):
 folder=OUT/layout/'shots';folder.mkdir(parents=True,exist_ok=True)
 wide=layout=='landscape';w,h=(1920,1080) if wide else (1080,1920)
 rows=json.loads((OUT/'narration.json').read_text())
 for row in rows:
  if len(sys.argv)>2 and row['name']!=sys.argv[2]:continue
  parts=[];nf=round(row['duration']*30);count=len(row['clips'])
  for j,clip in enumerate(row['clips']):
   source=OUT/'web-capture/live.mp4' if clip=='web' else OLD/layout/'shots'/f'{clip}.mp4';d=probe(source);n=nf//count+(j<nf%count);duration=n/30;start=0
   name=row['name'];crop=''
   if name=='world':d=1.1
   if name in ['flash_fly','flash_meal','flash_face']:start=.6;d=.7
   if name=='flash_fly':start=2
   if name=='alive':d*=.8
   if name=='outro':start=d*.8;d*=.2
   if name in ['meal','meal_pause','flash_meal','flower','flower_pause','groom','face','face_pause','flash_face']:
    # Tight portrait/landscape framing, without changing recorded poses.
    factor=1.15 if clip in ['meal','flower'] else 1.20
    cw=int(w/factor)//2*2;ch=int(h/factor)//2*2
    crop=f'crop={cw}:{ch}:{(w-cw)//2}:{(h-ch)//2},scale={w}:{h},'
   if name in ['face','face_pause']:start=1.75;d=min(d-start,1.9)
   if name=='flash_face':start=1.75;d=.7
   if name=='groom':d=min(d,2.1)
   if name=='eyes_full':
    x,y,cw,ch=(1130,230,720,410) if wide else (70,1080,940,535)
    crop=f'crop={cw}:{ch}:{x}:{y},scale={w}:{h}:force_original_aspect_ratio=decrease:flags=neighbor,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:color=0x10191c,'
   if name=='web':crop=f'scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:color=0x10191c,'
   # A mild grade lifts the dark fly, especially red eyes; no simulation lighting changes.
   vf=f'trim=start={start}:duration={d},setpts=(PTS-STARTPTS)*{duration/d},fps=30,{crop}eq=brightness=0.028:gamma=1.13:saturation=1.09,tpad=stop_mode=clone:stop_duration=0.1,format=yuv420p'
   part=folder/f'{name}-{j}.mp4';run(['ffmpeg','-v','error','-y','-i',str(source),'-vf',vf,'-frames:v',str(n),'-an','-c:v','libx264','-threads','4','-preset','fast','-crf','18',str(part)]);parts.append(part)
  listing=folder/f'{row["name"]}.concat';listing.write_text(''.join(f"file '{p.name}'\n" for p in parts))
  run(['ffmpeg','-v','error','-y','-f','concat','-safe','0','-i',str(listing),'-c','copy',str(folder/f'{row["name"]}.mp4')]);print(layout,row['name'],'DONE',flush=True)
if __name__=='__main__':
 if sys.argv[1]=='voice':asyncio.run(voices())
 else:edit(sys.argv[1])
