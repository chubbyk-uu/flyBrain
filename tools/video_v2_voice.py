import asyncio,json,os,subprocess,math
from pathlib import Path
import edge_tts
OUT=Path('outputs/social-video-v2');(OUT/'audio').mkdir(parents=True,exist_ok=True)
LINES=[('hook','我在电脑里，养了一只赛博果蝇。'),('not_animation','但它不是按剧本播放的动画。'),
('wiring','它的神经网络，来自科学家最近公布的真实果蝇神经连接图。'),
('scan','这张图，是通过扫描和重建，一点点拼出来的。'),
('hybrid','我把这张网络接进物理世界，给它身体，也加上饥饿、疲劳这些规则。'),
('busy','然后，它开始忙起来了。'),('walk','六条腿到处逛。'),('takeoff','拍动翅膀飞起来。'),
('flight','飞累了，再落下来歇一会儿。'),('sugar','饿了，就顺着气味找糖水。'),('meal','伸出小嘴，吃上一口。'),
('flower','花心里的花蜜，也不放过。'),('groom','闲下来的时候，还会搓搓前脚，擦擦眼睛。'),
('retina','这是它左右眼里的模拟画面。'),('neural','而这些起伏，来自正在运行的神经网络。'),
('honest','它当然还不完美，离真正的数字生命，也还有很远。'),
('alive','但看着它忙碌的样子，你会觉得，它已经有一部分，活了过来。'),
('end','我的电脑里，好像真的住进了一个小家伙。')]
async def main():
 rows=[]
 for name,text in LINES:
  raw=OUT/'audio'/f'{name}-raw.mp3';audio=OUT/'audio'/f'{name}.mp3';meta=OUT/'audio'/f'{name}.words.jsonl'
  if not meta.exists():await edge_tts.Communicate(text,'zh-CN-XiaoxiaoNeural',rate='+2%',boundary='WordBoundary',proxy=os.environ.get('HTTPS_PROXY')).save(str(raw),str(meta))
  words=[json.loads(x) for x in meta.read_text().splitlines()];end=max((w['offset']+w['duration'])/1e7 for w in words)
  duration=math.ceil((end+.38)*30)/30
  subprocess.run(['ffmpeg','-v','error','-y','-i',str(raw),'-t',str(duration),'-ar','48000','-b:a','192k',str(audio)],check=True)
  rows.append(dict(name=name,text=text,audio=str(audio),duration=duration,voice_duration=end))
  print(name,duration,flush=True)
 (OUT/'narration.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2));print('TOTAL',sum(r['duration'] for r in rows),flush=True)
asyncio.run(main())
