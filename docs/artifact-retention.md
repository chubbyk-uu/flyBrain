# Artifact retention and cleanup

Keep source code, fixtures, assets/manifests/licenses, packs, experiment JSON/logs,
published evidence, recorded simulation sources, video dependency shots and accepted
final MP4s. Keep negative results as well as passing results. Never use a broad
`git clean` or delete `outputs/` wholesale: it contains data absent from Git.

New generated output and `web/node_modules/` are ignored. Existing tracked evidence
remains tracked. New evidence intended for publication must be reviewed and explicitly
added with `git add -f PATH`; do not commit arbitrary multi-gigabyte recordings.

The cleanup tool defaults to a dry run. It only selects untracked numeric PNG frames
under `outputs/indoor-v2/` that are not explicitly referenced by project documentation
or source. It preserves every tracked file and non-frame report, plus video assets.
Sequence frames are regenerable from native replay data; individually cited frames
remain in place. Directory-only references may no longer contain every intermediate
frame after cleanup, but their cited evidence and native recordings remain.

```bash
python tools/cleanup_generated.py
# Use a new, empty archive directory outside this repository:
python tools/cleanup_generated.py --apply --archive-dir /path/to/cleanup-archive
```

Apply creates a compressed, hash-verified recovery archive and manifest before
removing exact selected paths. It does not change simulation behavior, stop processes,
delete data packs, touch Windows Downloads or clear build/dependency caches. Restore
the archive relative to the repository only after checking for newer files to avoid
overwriting them. Archive retention/reclamation is a separate user decision.

## 2026-09-13 inventory

Removed 5,733 untracked intermediate PNG frames (1,343,410,166 bytes) from the
working project after verifying the recovery archive. Also retired the one-off
`audition_video_voice.py` and `film_disable_unused_browser_retina.mjs` helpers into
that external recovery folder. The former is superseded by the retained multi-voice
audition helper; the latter patched already-open film tabs and is no longer needed.
No tracked acceptance evidence, raw numerical reports, final videos or data packs
were deleted. The recovery archive means this reduces project clutter, not net disk
usage; reclaiming the backup's space requires a separate deletion decision.
