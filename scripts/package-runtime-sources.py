"""Package only verified corresponding sources, alongside their provenance."""
from pathlib import Path
import hashlib
import json
import zipfile

root = Path(__file__).resolve().parent.parent
source = root / '.cache/runtime-sources'
manifest = json.loads((source / 'manifest.json').read_text())
expected = json.loads((root / 'docs/runtime-sources.json').read_text())
if manifest != expected or any('error' in row for row in manifest):
    raise SystemExit('Source collection does not match the checked-in manifest')
destination = root / 'dist/winshell-runtime-sources-2.55.0.5.zip'
destination.parent.mkdir(exist_ok=True)
with zipfile.ZipFile(destination, 'x', compression=zipfile.ZIP_STORED) as archive:
    for row in manifest:
        path = source / row['file']
        with path.open('rb') as stream:
            if hashlib.file_digest(stream, 'sha256').hexdigest() != row['sha256']:
                raise SystemExit('Source checksum mismatch: ' + row['file'])
        archive.write(path, 'sources/' + row['file'])
    archive.write(root / 'docs/runtime-sources.json', 'runtime-sources.json')
    archive.write(root / 'docs/runtime-package-versions.txt', 'runtime-package-versions.txt')
    archive.write(root / 'THIRD_PARTY_NOTICES.md', 'THIRD_PARTY_NOTICES.md')
with destination.open('rb') as stream:
    checksum = hashlib.file_digest(stream, 'sha256').hexdigest()
Path(str(destination)+'.sha256').write_text(checksum + '  ' + destination.name + '\n')
print(destination)
