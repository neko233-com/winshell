"""Collect pinned upstream source archives without executing any package code.

Based on the package-base mapping and official mirrors documented in
https://github.com/git-for-windows/build-extra/blob/main/get-sources.sh
The generated manifest records every binary package and the source archive URL
and SHA256; a missing archive is a hard failure, never silently omitted.
"""
import concurrent.futures
import hashlib
import json
from pathlib import Path
import urllib.request
import urllib.error

ROOT = Path(__file__).resolve().parent.parent
DEST = ROOT / '.cache' / 'runtime-sources'
DEST.mkdir(parents=True, exist_ok=True)


def base_name(name):
    if name.startswith('mingw-w64-x86_64-'):
        suffix = name.removeprefix('mingw-w64-x86_64-')
        if suffix in ('git-doc-html', 'git-credential-wincred', 'git-for-windows-addons',
                      'git-gui', 'git-perl', 'git-send-email', 'git-subtree', 'gitk'):
            suffix = 'git'
        suffix = {'gcc-libs': 'gcc', 'curl-winssl': 'curl', 'curl-openssl-alternate': 'curl',
                  'libssh2-wincng': 'libssh2', 'libwinpthread': 'winpthreads',
                  'gettext-runtime': 'gettext'}.get(suffix, suffix)
        return 'mingw-w64-' + suffix, 'mingw-w64-x86_64-' + suffix, 'mingw'
    aliases = {'gcc-libs':'gcc', 'heimdal-libs':'heimdal', 'libbz2':'bzip2',
        'libcurl':'curl', 'libdb':'db', 'libexpat':'expat', 'libgdbm':'gdbm',
        'libgnutls':'gnutls', 'libhogweed':'nettle', 'iconv':'libiconv',
        'libintl':'gettext', 'liblz4':'lz4', 'liblzma':'xz', 'libnettle':'nettle',
        'libnghttp2':'nghttp2', 'libnpth':'npth', 'libopenssl':'openssl',
        'libp11-kit':'p11-kit', 'libpcre':'pcre', 'libpcre2_8':'pcre2',
        'libreadline':'readline', 'libsasl':'cyrus-sasl', 'libserf':'serf',
        'libsqlite':'sqlite', 'libutil-linux':'util-linux', 'libzstd':'zstd'}
    name = aliases.get(name, name)
    return name, name, 'msys'


def fetch(url):
    request = urllib.request.Request(url, headers={'User-Agent': 'WinShell-source-collector'})
    return urllib.request.urlopen(request, timeout=90)


def collect(item):
    (base, version, index_name, kind), packages = item
    filename = f'{base}-{version}.src.tar.gz'
    for cached_name in [filename, filename.removesuffix('.gz')+'.zst']:
        path = DEST / cached_name
        record_path = DEST / (cached_name + '.json')
        if path.exists() and record_path.exists():
            record = json.loads(record_path.read_text())
            if hashlib.file_digest(path.open('rb'), 'sha256').hexdigest() == record['sha256']:
                record['packages'] = packages
                return record
    urls = []
    try:
        with fetch(f'https://raw.githubusercontent.com/git-for-windows/pacman-repo/refs/heads/x86_64/{index_name}.versions.json') as response:
            tag = json.load(response).get(version)
        if tag:
            urls.append(f'https://github.com/git-for-windows/pacman-repo/releases/download/{tag}/{filename}')
    except (OSError, ValueError):
        pass
    urls.extend([f'https://repo.msys2.org/{kind}/sources/{filename.removesuffix(".gz")}.zst',
                 f'https://repo.msys2.org/{kind}/sources/{filename}',
                 f'https://wingit.blob.core.windows.net/sources/{filename}'])
    errors = []
    for url in urls:
        try:
            downloaded_name = url.rsplit('/', 1)[-1]
            path = DEST / downloaded_name
            record_path = DEST / (downloaded_name + '.json')
            with fetch(url) as response, path.with_suffix('.part').open('wb') as output:
                while chunk := response.read(1024 * 1024):
                    output.write(chunk)
            path.with_suffix('.part').replace(path)
            record = {'packages': packages, 'file': downloaded_name, 'url': url,
                      'sha256': hashlib.file_digest(path.open('rb'), 'sha256').hexdigest()}
            record_path.write_text(json.dumps(record, indent=2)+'\n')
            return record
        except OSError as error:
            errors.append(str(error))
    return {'packages': packages, 'file': filename, 'error': errors}


def main():
    groups = {}
    versions = ROOT / 'runtime/git/etc/package-versions.txt'
    for line in versions.read_text().splitlines():
        name, version = line.split()
        base, index_name, kind = base_name(name)
        groups.setdefault((base, version, index_name, kind), []).append(line)
    records = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
        for index, record in enumerate(pool.map(collect, groups.items()), 1):
            records.append(record)
            if 'error' in record:
                print('MISSING', record['file'], flush=True)
            elif index % 10 == 0:
                print(f'Sources {index}/{len(groups)}', flush=True)
    (DEST / 'manifest.json').write_text(json.dumps(records, indent=2)+'\n')
    missing = [record for record in records if 'error' in record]
    print(f'{len(records)-len(missing)}/{len(records)} source packages collected', flush=True)
    if missing:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
