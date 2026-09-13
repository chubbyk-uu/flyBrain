"""Independent ASR sanity check; not proof of perfect pronunciation."""
import json
from pathlib import Path
from faster_whisper import WhisperModel
out=Path('outputs/social-video-20260913')
model=WhisperModel('small',device='cpu',compute_type='int8',cpu_threads=6)
segments,info=model.transcribe(str(out/'audio/narration.wav'),language='zh',beam_size=5,vad_filter=True)
rows=[dict(start=s.start,end=s.end,text=s.text) for s in segments]
(out/'voice-asr-check.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2))
for r in rows:print(r,flush=True)
