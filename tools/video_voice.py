"""Chinese male narration; plain-language script avoids ambiguous technical readings."""
import asyncio,json,os,subprocess
from pathlib import Path
import edge_tts
OUT=Path('outputs/social-video-20260913')
LINES=[
 ('hook','我在电脑里，养了一只赛博果蝇。'),
 ('world','它的小世界，只有一张茶几，两份甜点。'),
 ('walk','但它可不会乖乖待着。六条腿，先逛一圈。'),
 ('takeoff','下一秒，振翅起飞。桌面，变成了它的停机坪。'),
 ('flight','飞一会儿，再落下来。今天的行程，它边走边定。'),
 ('sugar','肚子饿了？顺着气味，去找这滴糖水。'),
 ('meal','靠近还不算吃到。看这里，嘴伸出来了，真的在进食。'),
 ('flower','另一边，花心里也有一口甜的。'),
 ('groom','吃饭之外，它还有小动作。前脚搓一搓，再擦擦眼睛。'),
 ('retina','换个视角。这是它左右两只眼睛的模拟画面。'),
 ('neural','这些跳动的曲线，则来自正在运行的神经仿真。'),
 ('explain','背后，是根据真实果蝇神经连接结构，搭起的网络。'),
 ('hybrid','再接上物理身体，加上饥饿、疲劳这些工程规则。'),
 ('honest','所以，这不是预先画好的动画。也不是完整复制了一只真果蝇。'),
 ('end','离数字生命还很远。但我已经会停下来，看这个小家伙，今天又在忙什么。'),
]
async def main():
 result=[]
 for name,text in LINES:
  path=OUT/'audio'/f'{name}.mp3'
  metadata=OUT/'audio'/f'{name}.words.jsonl'
  if not metadata.exists():
   voice=edge_tts.Communicate(text,'zh-CN-YunxiNeural',rate='+4%',boundary='WordBoundary',proxy=os.environ.get('HTTPS_PROXY'))
   await voice.save(str(path),str(metadata))
  duration=float(subprocess.check_output(['ffprobe','-v','error','-show_entries','format=duration','-of','csv=p=0',str(path)]))
  result.append(dict(name=name,text=text,audio=str(path),voice_duration=duration,duration=round(duration+.22,3)))
 (OUT/'narration.json').write_text(json.dumps(result,ensure_ascii=False,indent=2))
 print('Narration duration:',sum(r['duration'] for r in result),flush=True)
asyncio.run(main())
