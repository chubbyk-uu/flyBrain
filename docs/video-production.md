# Social video production

Accepted local delivery: V5, 71.833 seconds, 30 FPS, Chinese Xiaoxiao narration,
burned-in Chinese subtitles, quiet original keyboard score, no sound effects.
Portrait is 1080×1920; landscape is 1920×1080. V4 imagery is preserved and retimed.

Local files (generated, excluded from Git):

- `outputs/social-video-v5/portrait/cyber-fly-portrait-xiaoxiao.mp4`
- `outputs/social-video-v5/landscape/cyber-fly-landscape-xiaoxiao.mp4`
- `outputs/social-video-v5/narration.json`, `voice-config.json`, `delivery-validation.json`

Both were also copied to the user's Windows Downloads folder. Final video SHA-256:

```text
portrait  5eebeb88b9cfe2cd9eeaa0f70225eeaee4e504530b198df8f55ed5bd4d04f265
landscape 1f38b45155f037a11d6580d8c9faddb8298cd098b351eb7428b6e7a776ee31ac
```

## Reproduce from retained V4 footage

Requires ffmpeg/ffprobe, Microsoft YaHei (or intentionally change the caption font),
Python NumPy and `edge-tts`; optional ASR uses `faster-whisper` and its small model.
Use the dedicated environment and existing HTTPS_PROXY for TTS downloads. No API
credential is embedded. Online voice availability can change; retain generated audio.

```bash
python tools/video_v5.py voice
python tools/video_v5.py portrait
python tools/video_v5.py landscape
FLYBRAIN_VIDEO_OUT=outputs/social-video-v5 FLYBRAIN_VIDEO_LAYOUT=portrait python tools/finish_social_video.py --render
FLYBRAIN_VIDEO_OUT=outputs/social-video-v5 FLYBRAIN_VIDEO_LAYOUT=landscape python tools/finish_social_video.py --render
FLYBRAIN_VIDEO_OUT=outputs/social-video-v5 python tools/check_video_v2_voice.py
python tools/validate_video_v5.py
```

These commands overwrite their named generated outputs; preserve an accepted copy
first. TTS caches word boundaries/audio: remove the matching cache only when changing
its text or voice. A fresh clone does not contain the necessary footage; V5 is not a
self-contained asset download tool. Preserve V4 raw shots and validation manifest.

## Production lineage and boundaries

`video_v3.py` supplies the shared narration helper; V4 supplies fresh bright footage;
V5 changes the voice/ending. Older validators and V1–V3 source material are retained
because V4 includes inherited stereo, webpage and neural shots. They are dependencies,
not duplicate scripts safe to delete indiscriminately.

`render_video_v2.mjs` and `web/film-studio-v2.*` replay recorded native poses through
Three.js. Recording assumes an isolated browser with local debugging port 9338 and
static server port 8080. `examples/film_stereo_capture.rs` captures both native eyes,
body poses and neural telemetry at the same unadvanced state with separate display
processors. This is offline presentation, not a change to the live sensory path.

The final validation checked dimensions, 2,155 frames, full decoding, identical
audio across layouts, exact approved ending, silent SFX track and audio levels
(-15.2 LUFS, -0.9 dBFS true peak). Contact sheets were inspected. ASR is a sanity
check, not a guarantee of every pronunciation; no direct listening was available.
Edited camera playback is not evidence of live realtime speed or biological autonomy.
Source and asset licensing remains governed by [third-party notices](../THIRD_PARTY_NOTICES.md).
