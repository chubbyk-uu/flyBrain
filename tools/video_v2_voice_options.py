"""Equal-text, equal-loudness auditions for the user's voice selection."""
import asyncio,json,os,subprocess
from pathlib import Path
import edge_tts
OUT=Path('outputs/social-video-v2/voice-options')
TEXT='我在电脑里，养了一只赛博果蝇。但它不是预先设置好的动画。它的神经网络，来自真实果蝇的神经连接图。虽然还不完美，但看着它忙碌的样子，你会觉得，它仿佛已经有一部分，活了过来。'
OPTIONS=[('A','Xiaoxiao','女声 · 晓晓'),('B','Xiaoyi','女声 · 晓伊'),('C','Yunyang','男声 · 云扬'),('D','Yunjian','男声 · 云健'),('E','Yunxia','男声 · 云夏')]
async def main():
 OUT.mkdir(parents=True,exist_ok=True);rows=[]
 for key,name,label in OPTIONS:
  raw=OUT/f'{key}-{name}-raw.mp3';dest=OUT/f'{key}-{name}.mp3'
  await edge_tts.Communicate(TEXT,f'zh-CN-{name}Neural',rate='+2%',proxy=os.environ.get('HTTPS_PROXY')).save(str(raw))
  subprocess.run(['ffmpeg','-v','error','-y','-i',str(raw),'-af','loudnorm=I=-16:TP=-2:LRA=8','-ar','48000','-b:a','192k',str(dest)],check=True)
  duration=float(subprocess.check_output(['ffprobe','-v','error','-show_entries','format=duration','-of','csv=p=0',str(dest)]))
  rows.append(dict(option=key,voice=f'zh-CN-{name}Neural',label=label,text=TEXT,path=str(dest),duration=duration));print(label,round(duration,2),flush=True)
 (OUT/'options.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2))
asyncio.run(main())
