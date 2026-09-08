"""Generate notices and a dependency inventory from the locked Cargo graph."""
import json
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parent.parent
output = Path(sys.argv[1]) if len(sys.argv) > 1 else root / '.cache' / 'licenses'
output.mkdir(parents=True, exist_ok=True)
metadata = json.loads(subprocess.check_output(
    ['cargo', 'metadata', '--format-version', '1', '--locked'], cwd=root))
inventory = []
notices = ['WinShell Rust dependency notices\nVersions resolved from Cargo.lock.\n']
for package in sorted(metadata['packages'], key=lambda p: (p['name'], p['version'])):
    if package['source'] is None:
        continue
    inventory.append({key: package.get(key) for key in ('name', 'version', 'license', 'source', 'repository')})
    path = Path(package['manifest_path']).parent
    notices.append('\n' + '='*72 + '\n' + package['name'] + ' ' + package['version'] +
                   '\nLicense expression: ' + str(package['license']) + '\n')
    files = [file for file in path.iterdir() if file.is_file() and
             file.name.upper().startswith(('LICENSE', 'LICENCE', 'NOTICE', 'COPYING', 'COPYRIGHT', 'UNLICENSE'))]
    if package.get('license_file'):
        files.append(path / package['license_file'])
    for directory in ('license', 'licenses'):
        if (path / directory).is_dir():
            files.extend(file for file in (path / directory).rglob('*') if file.is_file())
    for file in sorted(set(files)):
        notices.append('\n--- ' + file.relative_to(path).as_posix() + ' ---\n')
        notices.append(file.read_text(encoding='utf-8', errors='replace'))
    if not files:
        notices.append('Source and license: https://crates.io/crates/' + package['name'] + '/' + package['version'] + '\n')
(output / 'RUST_DEPENDENCIES.json').write_text(json.dumps(inventory, indent=2)+'\n', encoding='utf-8')
(output / 'RUST_LICENSES.txt').write_text('\n'.join(notices), encoding='utf-8')
print(f'Collected {len(inventory)} Rust dependency notices in {output}')
