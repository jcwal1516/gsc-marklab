#!/usr/bin/env python3
from pathlib import Path
import hashlib, json, sys

root = Path(__file__).resolve().parents[2]
docs = root / 'docs' / 'implementation'
manifest_path = docs / 'PSEUDOCODE_MANIFEST.json'
manifest = json.loads(manifest_path.read_text(encoding='utf-8'))

errors = []
parts = []
for entry in sorted(manifest['split_files'], key=lambda x: x['order']):
    path = root / entry['file']
    if not path.exists():
        errors.append(f'missing: {entry["file"]}')
        continue
    data = path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    if digest != entry['sha256']:
        errors.append(f'digest mismatch: {entry["file"]} expected={entry["sha256"]} actual={digest}')
    parts.append(data)

full_path = root / manifest['canonical_full_file']
if not full_path.exists():
    errors.append(f'missing: {manifest["canonical_full_file"]}')
else:
    full = full_path.read_bytes()
    digest = hashlib.sha256(full).hexdigest()
    if digest != manifest['canonical_full_sha256']:
        errors.append(f'full digest mismatch: expected={manifest["canonical_full_sha256"]} actual={digest}')
    if b''.join(parts) != full:
        errors.append('ordered split-file concatenation is not byte-identical to the canonical full file')
    if len(full.decode('utf-8').splitlines()) != manifest['canonical_full_line_count']:
        errors.append('canonical line-count mismatch')

if errors:
    print('PSEUDOCODE PACK VERIFICATION FAILED', file=sys.stderr)
    for error in errors:
        print(f'- {error}', file=sys.stderr)
    raise SystemExit(1)

print('PSEUDOCODE PACK VERIFIED')
print(f'full_sha256={manifest["canonical_full_sha256"]}')
print(f'split_files={len(manifest["split_files"])}')
print(f'lines={manifest["canonical_full_line_count"]}')
