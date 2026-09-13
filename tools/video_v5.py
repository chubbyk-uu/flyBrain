"""Voice-only revision: approved ending, Xiaoxiao, preserved V4 footage."""
import asyncio,json,subprocess,sys,shutil
from pathlib import Path
import video_v3 as base

ROOT=Path('outputs/social-video-v5')
ENDING='它当然还不是真正的生命。但当这只果蝇开始感知环境，神经活动也开始决定下一步做什么时……至少，它的一部分已经活过来了，不是吗？'
REPLACE={
 'web':'这就是它住的网页。',
 'emotion':'但当这只果蝇开始感知环境，神经活动也开始决定下一步做什么时……',
 'alive':'至少，它的一部分已经活过来了，不是吗？',
}
def voice():
 base.OUT=ROOT
 base.VOICE='zh-CN-XiaoxiaoNeural'
 base.PLAN=[(n,REPLACE.get(n,t),c,d,p) for n,t,c,d,p in base.PLAN]
 assert ''.join(t for n,t,*_ in base.PLAN if n in ['honest','emotion','alive'])==ENDING
 asyncio.run(base.voices())
 (ROOT/'voice-config.json').write_text(json.dumps({'voice':base.VOICE,'rate':'+2%','ending':ENDING},ensure_ascii=False,indent=2))

def edit(layout):
 folder=ROOT/layout/'shots';folder.mkdir(parents=True,exist_ok=True)
 rows=json.loads((ROOT/'narration.json').read_text())
 for row in rows:
  source=Path('outputs/social-video-v4')/layout/'shots'/f"{row['name']}.mp4"
  target=folder/source.name
  info=json.loads(subprocess.check_output(['ffprobe','-v','error','-select_streams','v:0','-show_entries','stream=nb_frames','-of','json',str(source)]))
  oldframes=int(info['streams'][0]['nb_frames']);frames=round(row['duration']*30)
  if oldframes==frames:shutil.copy2(source,target)
  else:
   # Preserve camera/pose order; only adapt playback to the new spoken duration.
   base.run(['ffmpeg','-v','error','-y','-i',str(source),'-vf',f'setpts=(PTS-STARTPTS)*{frames/oldframes},fps=30,tpad=stop_mode=clone:stop_duration=0.2,format=yuv420p','-frames:v',str(frames),'-an','-c:v','libx264','-threads','4','-preset','fast','-crf','18',str(target)])
  print(layout,row['name'],frames,flush=True)
if __name__=='__main__':
 if sys.argv[1]=='voice':voice()
 else:edit(sys.argv[1])
