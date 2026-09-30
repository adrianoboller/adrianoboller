#!/usr/bin/env python3
from __future__ import annotations
import hashlib, json, sys, zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open('rb') as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()

def fail(msg: str) -> None:
    raise SystemExit(f"FAIL: {msg}")

def main() -> int:
    if len(sys.argv) != 3:
        fail('usage: scripts/plugin_pack.py <plugin-root> <output.phxplugin>')
    plugin_root = Path(sys.argv[1]).resolve()
    output = Path(sys.argv[2]).resolve()
    if not plugin_root.is_dir():
        fail(f'plugin root not found: {plugin_root}')
    manifests = list(plugin_root.glob('*.plugin.json'))
    if len(manifests) != 1:
        fail('plugin root must contain exactly one *.plugin.json')
    manifest_path = manifests[0]
    manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
    integrity = manifest.get('integrity', {})
    for field in ('digest', 'signature', 'signer', 'provenance'):
        value = str(integrity.get(field, '')).strip()
        if not value or value.startswith('<'):
            fail(f'manifest integrity.{field} is not finalized')
    artifact = (ROOT / integrity['artifact']).resolve()
    if not artifact.is_file():
        fail(f'artifact missing: {artifact}')
    if sha256(artifact).lower() != integrity['digest'].lower():
        fail('artifact digest does not match manifest')

    required = {'README.md', 'LICENSE', 'SBOM.spdx.json'}
    missing = [name for name in required if not (plugin_root / name).is_file()]
    if missing:
        fail(f'missing package files: {missing}')

    files = []
    for path in sorted(p for p in plugin_root.rglob('*') if p.is_file()):
        rel = path.relative_to(plugin_root).as_posix()
        files.append({'path': rel, 'sha256': sha256(path)})

    descriptor = {
        'format': 'phxclaw-plugin-package-v1',
        'manifest': manifest_path.name,
        'license': 'LICENSE',
        'readme': 'README.md',
        'sbom': 'SBOM.spdx.json',
        'files': files,
    }
    descriptor_path = plugin_root / 'PACKAGE.json'
    descriptor_path.write_text(json.dumps(descriptor, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')

    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(output, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as zf:
        for path in sorted(p for p in plugin_root.rglob('*') if p.is_file()):
            zf.write(path, path.relative_to(plugin_root).as_posix())
    print(json.dumps({'package': str(output), 'sha256': sha256(output), 'files': len(files) + 1}, indent=2))
    return 0

if __name__ == '__main__':
    raise SystemExit(main())
