"""Conservative cleanup: dry-run by default; verify recovery archive before unlink."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parents[1]


def candidates():
    tracked = set(subprocess.check_output(
        ['git', 'ls-files', '-z'], cwd=ROOT).decode().split('\0'))
    references = []
    for folder in ['docs', 'tools', 'web', 'examples']:
        for path in (ROOT / folder).glob('**/*'):
            if 'node_modules' in path.parts or path.is_symlink():
                continue
            if path.suffix in {'.md', '.py', '.mjs', '.js', '.rs', '.html'}:
                references.append(path.read_text(errors='replace'))
    reference_text = '\n'.join(references)
    selected = []
    for path in sorted((ROOT / 'outputs/indoor-v2').rglob('*.png')):
        relative = path.relative_to(ROOT).as_posix()
        if path.is_symlink() or not re.fullmatch(r'\d+', path.stem):
            continue
        if relative in tracked or relative in reference_text:
            continue
        selected.append(path)
    return selected


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--apply', action='store_true')
    parser.add_argument('--archive-dir', type=Path)
    args = parser.parse_args()
    paths = candidates()
    print(json.dumps({'files': len(paths), 'bytes': sum(p.stat().st_size for p in paths),
                      'apply': args.apply}, indent=2), flush=True)
    if not args.apply:
        return
    if args.archive_dir is None:
        parser.error('--apply requires --archive-dir')
    archive_dir = args.archive_dir.resolve()
    if archive_dir == ROOT or ROOT in archive_dir.parents:
        parser.error('recovery archive must be outside the repository')
    archive_dir.mkdir(parents=True, exist_ok=True)
    if any(archive_dir.iterdir()):
        parser.error('archive directory must be empty')
    manifest = [{'path': p.relative_to(ROOT).as_posix(), 'bytes': p.stat().st_size,
                 'sha256': hashlib.sha256(p.read_bytes()).hexdigest()} for p in paths]
    archive = archive_dir / 'generated-frames.tar.gz'
    with tarfile.open(archive, 'x:gz') as tar:
        for row in manifest:
            tar.add(ROOT / row['path'], arcname=row['path'], recursive=False)
    with tarfile.open(archive, 'r:gz') as tar:
        assert len(tar.getmembers()) == len(manifest)
        for row in manifest:
            assert hashlib.sha256(tar.extractfile(row['path']).read()).hexdigest() == row['sha256']
    (archive_dir / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    for row in manifest:
        path = ROOT / row['path']
        # Refuse deletion if another process changed the file during archival.
        assert not path.is_symlink()
        assert hashlib.sha256(path.read_bytes()).hexdigest() == row['sha256']
        path.unlink()
    print(f'Removed {len(manifest)} regenerable frames; recovery: {archive}', flush=True)


if __name__ == '__main__':
    main()
