"""ASR sanity check of the new female narration, not a pronunciation guarantee."""
import json,os
from pathlib import Path
from faster_whisper import WhisperModel
out=Path(os.environ.get('FLYBRAIN_VIDEO_OUT','outputs/social-video-v2'))
model=WhisperModel('small',device='cpu',compute_type='int8',cpu_threads=4)
segments,_=model.transcribe(str(out/'portrait/audio/narration.wav'),language='zh',beam_size=5,vad_filter=True)
rows=[dict(start=s.start,end=s.end,text=s.text) for s in segments]
(out/'voice-asr-check.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2))
for r in rows:print(r,flush=True)
