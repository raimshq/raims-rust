"""Verify canonical crate bytes and publish stable Rust releases to crates.io."""
import argparse
import hashlib
import gzip
import io
import json
import os
from pathlib import Path
import re
import stat
import struct
import tarfile
import tomllib
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import zipfile

sys.dont_write_bytecode = True
REPOSITORY = 'raimshq/raims-rust'
WORKFLOW = '.github/workflows/validate.yml'
JOBS = {'validateRustMinimum': ('rust-sdk-check-minimum', 'Run the canonical Rust SDK checks'),
        'validateRustCurrent': ('rust-sdk-check-current', 'Run the canonical Rust SDK checks')}
RUNTIME_VERSIONS = {'minimum': '1.90.0', 'current': '1.99.1'}
SHA = re.compile(r'[a-f0-9]{40}')
VERSION = re.compile(r'(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)')
MAX_DOWNLOAD = 32 * 1024 * 1024
MAX_METADATA = 2 * 1024 * 1024
FILES = {'raims.crate', 'publish-metadata.json'}
MAX_CRATE = 10 * 1024 * 1024
MAX_PUBLISH_METADATA = 1024 * 1024

def require(condition, message):
    if not condition:
        raise RuntimeError(message)

def digest(raw):
    return hashlib.sha256(raw).hexdigest()

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, file, code, message, headers, new_url):
        return None

def read_response(request, maximum=MAX_METADATA):
    with urllib.request.build_opener(NoRedirect).open(request, timeout=60) as response:
        raw = response.read(maximum + 1)
        if response.headers.get('Content-Encoding') == 'gzip':
            require(len(raw) <= maximum, 'The compressed remote response exceeds the allowed size.')
            raw = gzip.decompress(raw)
    require(len(raw) <= maximum, 'The remote response exceeds the allowed size.')
    return raw

def public_download(url):
    parsed = urllib.parse.urlsplit(url)
    require(parsed.scheme == 'https' and parsed.hostname in {'github.com', 'crates.io', 'static.crates.io'}
            and parsed.username is None and parsed.password is None, 'Unexpected public artifact origin.')
    request = urllib.request.Request(url, headers={'User-Agent': 'Raims-Rust-Release'})
    try:
        return read_response(request, MAX_DOWNLOAD)
    except urllib.error.HTTPError as error:
        require(parsed.hostname == 'github.com' and error.code in {301, 302, 303, 307, 308},
                'A public artifact could not be read.')
        destination = error.headers.get('Location', '')
        redirect = urllib.parse.urlsplit(destination)
        require(redirect.scheme == 'https' and redirect.hostname == 'release-assets.githubusercontent.com'
                and redirect.username is None and redirect.password is None, 'Unexpected release download origin.')
        return read_response(urllib.request.Request(destination, headers={'User-Agent': 'Raims-Rust-Release'}), MAX_DOWNLOAD)

def github_url(endpoint):
    return ('https://api.github.com/repos/' + REPOSITORY + '/' + endpoint).rstrip('/')

def github(endpoint, binary=False):
    token = os.environ.get('GITHUB_TOKEN')
    require(token, 'A GitHub token is required for release verification.')
    headers = {'Authorization': 'Bearer ' + token, 'Accept': 'application/vnd.github+json',
               'User-Agent': 'Raims-Rust-Release', 'X-GitHub-Api-Version': '2022-11-28'}
    try:
        raw = read_response(urllib.request.Request(github_url(endpoint), headers=headers), MAX_DOWNLOAD if binary else MAX_METADATA)
    except urllib.error.HTTPError as error:
        require(binary and error.code in {301, 302, 303, 307, 308}, 'GitHub release metadata could not be read.')
        destination = error.headers.get('Location', '')
        url = urllib.parse.urlsplit(destination)
        require(url.scheme == 'https' and url.hostname and url.username is None and url.password is None and
                (url.hostname.endswith('.blob.core.windows.net') or url.hostname.endswith('.githubusercontent.com')
                 or url.hostname.endswith('.actions.githubusercontent.com')), 'Unexpected artifact download origin.')
        raw = read_response(urllib.request.Request(destination, headers={'User-Agent': 'Raims-Rust-Release'}), MAX_DOWNLOAD)
    return raw if binary else json.loads(raw)

def git(source, *args):
    p = subprocess.run(['git', '-C', str(source), *args], capture_output=True, text=True, timeout=60)
    require(p.returncode == 0, 'Release source identity could not be verified.')
    return p.stdout.strip()

def source_version(source):
    package = tomllib.loads((source / 'Cargo.toml').read_text(encoding='utf-8'))['package']
    require(package.get('name') == 'raims' and VERSION.fullmatch(package.get('version', ''))
            and package.get('rust-version') == '1.90' and package.get('repository') == 'https://github.com/' + REPOSITORY,
            'The official stable crate identity and minimum Rust version are required.')
    return package['version']


def context(source):
    require(os.environ.get('GITHUB_REPOSITORY') == REPOSITORY and os.environ.get('GITHUB_EVENT_NAME') == 'release',
            'Only the official repository release event may publish.')
    event = json.loads(Path(os.environ['GITHUB_EVENT_PATH']).read_text(encoding='utf-8'))
    release = event.get('release', {})
    require(event.get('action') == 'published' and release.get('draft') is False and release.get('prerelease') is False,
            'Only a published stable release can be uploaded.')
    version = source_version(source)
    tag = 'v' + version
    require(release.get('tag_name') == tag and os.environ.get('GITHUB_REF') == 'refs/tags/' + tag,
            'The release tag and crate version differ.')
    head = git(source, 'rev-parse', 'HEAD')
    require(SHA.fullmatch(head) and head == os.environ.get('GITHUB_SHA')
            and git(source, 'rev-parse', 'refs/tags/' + tag + '^{commit}') == head,
            'The checked-out release commit differs.')
    require(not git(source, 'status', '--porcelain', '--untracked-files=all'), 'Release sources must be clean.')
    repo = github('')
    require(repo.get('full_name') == REPOSITORY and repo.get('private') is False and repo.get('default_branch') == 'main',
            'The repository visibility or default branch differs.')
    require(github('commits/' + tag).get('sha') == head, 'The public tag changed.')
    git(source, 'merge-base', '--is-ancestor', head, 'origin/main')
    identifier = release.get('id')
    require(type(identifier) is int and identifier > 0, 'The release identity is invalid.')
    current = github('releases/' + str(identifier))
    require(current.get('id') == identifier and current.get('tag_name') == tag
            and current.get('draft') is False and current.get('prerelease') is False, 'The public release changed.')
    return {'sourceRevision': head, 'version': version, 'tag': tag, 'releaseId': identifier}

def validation(head, wait_seconds=600):
    deadline = time.monotonic() + wait_seconds
    while True:
        value = github('actions/workflows/validate.yml/runs?head_sha=' + head + '&per_page=100&page=1')
        rows = value.get('workflow_runs', [])
        require(isinstance(rows, list) and type(value.get('total_count')) is int, 'Validation history is incomplete.')
        if rows and rows[0].get('status') == 'completed':
            run = rows[0]
            break
        require(time.monotonic() < deadline, 'Validation of this exact commit did not finish within the allowed wait.')
        print('Waiting for validation of the release commit.', flush=True)
        time.sleep(min(15, max(0, deadline - time.monotonic())))
    require(run.get('head_sha') == head and run.get('path') == WORKFLOW and run.get('name') == 'Validate Rust SDK'
            and run.get('conclusion') == 'success' and run.get('event') in {'push', 'workflow_dispatch'}
            and run.get('repository', {}).get('full_name') == REPOSITORY
            and run.get('head_repository', {}).get('full_name') == REPOSITORY,
            'Successful validation of this exact commit is required.')
    identifier, attempt = run.get('id'), run.get('run_attempt')
    require(type(identifier) is int and identifier > 0 and type(attempt) is int and attempt > 0,
            'Validation attempt identity is incomplete.')
    response = github(f'actions/runs/{identifier}/attempts/{attempt}/jobs?per_page=100&page=1')
    jobs = response.get('jobs', [])
    require(response.get('total_count') == len(jobs) == len(JOBS) and {job.get('name') for job in jobs} == set(JOBS),
            'The complete Rust validation job inventory is required.')
    for job in jobs:
        suffix, step_name = JOBS[job['name']]
        require(job.get('status') == 'completed' and job.get('conclusion') == 'success'
                and set(job.get('labels', [])) == {f'codebuild-raims-github-actions-{identifier}-{attempt}', suffix}
                and len(job['labels']) == 2 and isinstance(job.get('runner_name'), str)
                and re.fullmatch(r'[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}', job['runner_name'])
                and job.get('runner_group_name'), 'A required AWS validation job did not pass.')
        required = [step for step in job.get('steps', []) if step.get('name') == step_name]
        require(len(required) == 1 and required[0].get('status') == 'completed' and required[0].get('conclusion') == 'success',
                'The canonical validation step did not pass.')
    return {'validationRunId': identifier, 'validationRunAttempt': attempt}

def artifact_files(value, proven):
    run, attempt = proven['validationRunId'], proven['validationRunAttempt']
    response = github(f'actions/runs/{run}/artifacts?per_page=100&page=1')
    require(type(response.get('total_count')) is int and 0 < response['total_count'] <= 100
            and isinstance(response.get('artifacts'), list) and len(response['artifacts']) == response['total_count'],
            'Validation artifact inventory is incomplete.')
    found = [row for row in response['artifacts'] if row.get('name') == f'raims-rust-validation-{run}-{attempt}']
    require(len(found) == 1 and found[0].get('expired') is False
            and found[0].get('workflow_run', {}).get('id') == run
            and found[0].get('workflow_run', {}).get('head_sha') == value['sourceRevision'], 'The approved crate artifact is missing.')
    artifact = found[0]
    require(type(artifact.get('id')) is int and artifact['id'] > 0, 'The artifact identity is invalid.')
    raw = github('actions/artifacts/' + str(artifact['id']) + '/zip', binary=True)
    require(artifact.get('digest') == 'sha256:' + digest(raw), 'The Actions artifact digest differs.')
    with zipfile.ZipFile(io.BytesIO(raw)) as archive:
        expected = {'release-manifest.json', 'dist/raims.crate', 'dist/publish-metadata.json'}
        require(set(archive.namelist()) == expected and len(archive.infolist()) == len(expected)
                and sum(item.file_size for item in archive.infolist()) <= MAX_DOWNLOAD
                and all(not item.is_dir() and not stat.S_ISLNK(item.external_attr >> 16) for item in archive.infolist()),
                'The canonical crate artifact contains unexpected entries.')
        files = {name: archive.read('dist/' + name) for name in FILES}
        manifest_raw = archive.read('release-manifest.json')
    manifest = json.loads(manifest_raw)
    require(set(manifest) == {'package', 'sourceRevision', 'toolkitRevision', 'runtimeVersions', 'files'}
            and manifest['package'] == {'name': 'raims', 'version': value['version']}
            and manifest['sourceRevision'] == value['sourceRevision']
            and manifest['toolkitRevision'] == os.environ.get('RAIMS_RUST_TOOLKIT_REF')
            and SHA.fullmatch(str(manifest['toolkitRevision']))
            and manifest['toolkitRevision'] != '0' * 40
            and manifest['runtimeVersions'] == RUNTIME_VERSIONS
            and manifest['files'] == {name: digest(raw) for name, raw in files.items()},
            'The canonical report and crate bytes differ.')
    require(files['publish-metadata.json'] == metadata_bytes(files['raims.crate'], value),
            'Upload metadata differs from the approved packaged Cargo manifest.')
    files['release-manifest.json'] = manifest_raw
    return files, artifact['id']


def upload_release_asset(identifier, name, raw):
    token = os.environ.get('GITHUB_TOKEN')
    require(token and type(identifier) is int and identifier > 0 and Path(name).name == name, 'An asset upload could not be authorized.')
    endpoint = 'https://uploads.github.com/repos/' + REPOSITORY + '/releases/' + str(identifier) + '/assets?name=' + urllib.parse.quote(name, safe='')
    request = urllib.request.Request(endpoint, data=raw, method='POST', headers={'Authorization': 'Bearer ' + token,
        'Content-Type': 'application/octet-stream', 'User-Agent': 'Raims-Rust-Release', 'Accept': 'application/vnd.github+json'})
    try:
        read_response(request)
    except (OSError, RuntimeError):
        current = github('releases/' + str(identifier))
        existing = [asset for asset in current.get('assets', []) if asset.get('name') == name]
        require(len(existing) == 1 and public_download(existing[0]['browser_download_url']) == raw,
                'Release asset creation was not confirmed; inspect the release before retrying.')

def release_files(value, files, create_missing=False):
    rows = github('releases/' + str(value['releaseId'])).get('assets', [])
    assets = {asset['name']: asset for asset in rows}
    expected = set(files) | {'SHA256SUMS'}
    require(len(assets) == len(rows) and set(assets).issubset(expected), 'The release contains unexpected or duplicate assets.')
    complete = dict(files, SHA256SUMS=''.join(digest(files[name]) + '  ' + name + '\n' for name in sorted(files)).encode())
    for name, raw in complete.items():
        if name in assets:
            require(public_download(assets[name]['browser_download_url']) == raw, 'An existing release asset differs from approved CI bytes.')
    missing = set(complete) - set(assets)
    require(not missing or create_missing, 'The approved crate assets are missing from the release.')
    for name in sorted(missing):
        upload_release_asset(value['releaseId'], name, complete[name])
    if missing:
        rows = github('releases/' + str(value['releaseId'])).get('assets', [])
        assets = {asset['name']: asset for asset in rows}
    require(len(assets) == len(rows) and set(assets) == expected, 'The release asset inventory differs.')
    for name, raw in complete.items():
        require(public_download(assets[name]['browser_download_url']) == raw, 'A public release asset differs.')


def metadata_bytes(raw, value):
    """Derive Cargo's registry request metadata from the approved archive only."""
    require(0 < len(raw) <= MAX_CRATE, 'The crate exceeds the reviewed registry size limit.')
    root = 'raims-' + value['version']
    with tarfile.open(fileobj=io.BytesIO(raw), mode='r:gz') as archive:
        members = archive.getmembers()
        names = [member.name for member in members]
        require(len(names) == len(set(names)) and sum(item.size for item in members) <= 64 * 1024 * 1024,
                'The crate contains duplicate or oversized entries.')
        for item in members:
            path = Path(item.name)
            require(not path.is_absolute() and '..' not in path.parts and chr(92) not in item.name
                    and item.name.startswith(root + '/') and item.isfile()
                    and not any(part.lower() in {'agents.md', 'todo.md', 'tests', 'test', '.github', '.git', 'scripts'}
                                for part in path.parts), 'The crate contains unexpected or internal entries.')
        def read(name):
            member = archive.extractfile(root + '/' + name)
            require(member is not None, 'A required packaged metadata file is missing.')
            return member.read()
        project = tomllib.loads(read('Cargo.toml').decode('utf-8'))
        vcs = json.loads(read('.cargo_vcs_info.json'))
        require(set(vcs) == {'git', 'path_in_vcs'} and vcs.get('path_in_vcs') == ''
                and isinstance(vcs.get('git'), dict) and set(vcs['git']).issubset({'sha1', 'dirty'})
                and vcs['git'].get('sha1') == value['sourceRevision'] and vcs['git'].get('dirty', False) is False,
                'The packaged VCS revision differs or the crate was built from dirty sources.')
        package = project['package']
        require(package.get('name') == 'raims' and package.get('version') == value['version']
                and package.get('repository') == 'https://github.com/' + REPOSITORY
                and package.get('rust-version') == '1.90' and package.get('license') == 'MIT'
                and package.get('readme') == 'README.md' and package.get('edition') == '2021',
                'The packaged crate identity differs.')
        readme = read('README.md').decode('utf-8')
    dependencies = []
    def collect(table, kind, target=None):
        require(isinstance(table, dict), 'The packaged dependency table is invalid.')
        for alias, dependency in table.items():
            if isinstance(dependency, str):
                dependency = {'version': dependency}
            require(isinstance(dependency, dict) and 'version' in dependency
                    and not (set(dependency) & {'path', 'git', 'registry', 'registry-index', 'branch', 'rev', 'tag'}),
                    'Only registry dependencies may be published.')
            name = dependency.get('package', alias)
            dependencies.append({'name': name, 'version_req': dependency['version'],
                'features': dependency.get('features', []), 'optional': dependency.get('optional', False),
                'default_features': dependency.get('default-features', True), 'target': target,
                'kind': kind, 'registry': None, 'explicit_name_in_toml': alias if name != alias else None})
    for key, kind in [('dependencies', 'normal'), ('dev-dependencies', 'dev'), ('build-dependencies', 'build')]:
        collect(project.get(key, {}), kind)
    for target, table in project.get('target', {}).items():
        for key, kind in [('dependencies', 'normal'), ('dev-dependencies', 'dev'), ('build-dependencies', 'build')]:
            collect(table.get(key, {}), kind, target)
    dependencies.sort(key=lambda item: (item['name'], item['kind'], item['target'] or ''))
    metadata = {'name': 'raims', 'vers': value['version'], 'deps': dependencies,
        'features': project.get('features', {}), 'authors': package.get('authors', []),
        'description': package.get('description'), 'documentation': package.get('documentation'),
        'homepage': package.get('homepage'), 'readme': readme, 'readme_file': 'README.md',
        'keywords': package.get('keywords', []), 'categories': package.get('categories', []),
        'license': package['license'], 'license_file': None, 'repository': package['repository'],
        'badges': {}, 'links': package.get('links'), 'rust_version': package['rust-version']}
    return (json.dumps(metadata, sort_keys=True, separators=(',', ':'), ensure_ascii=False) + '\n').encode('utf-8')


def public_json(url, missing=False):
    parsed = urllib.parse.urlsplit(url)
    require(parsed.scheme == 'https' and parsed.hostname in {'crates.io', 'index.crates.io'},
            'Unexpected public registry metadata origin.')
    try:
        return json.loads(read_response(urllib.request.Request(url, headers={
            'User-Agent': 'Raims-Rust-Release (https://github.com/' + REPOSITORY + ')',
            'Cache-Control': 'no-cache', 'Accept': 'application/json'})))
    except urllib.error.HTTPError as error:
        if missing and error.code == 404:
            return None
        raise RuntimeError('Public crates.io metadata could not be read.') from None


def public_version(version, raw):
    value = public_json('https://crates.io/api/v1/crates/raims/' + version, missing=True)
    if value is None:
        return None
    metadata = value.get('version', {})
    require(metadata.get('crate') == 'raims' and metadata.get('num') == version
            and metadata.get('checksum') == digest(raw) and metadata.get('yanked') is False
            and metadata.get('license') == 'MIT' and metadata.get('rust_version') == '1.90',
            'An immutable existing crate version differs from the approved identity or bytes.')
    owner = os.environ.get('CRATES_IO_OWNER', '')
    require(re.fullmatch(r'[A-Za-z0-9](?:[A-Za-z0-9-]{0,38})', owner), 'The reviewed crate owner is missing.')
    require(metadata.get('published_by', {}).get('login') == owner,
            'The crate was not published by the reviewed permanent credential account.')
    return metadata


def indexed(version, raw):
    metadata = public_version(version, raw)
    if metadata is None:
        return None
    package = public_json('https://crates.io/api/v1/crates/raims').get('crate', {})
    require(package.get('name') == 'raims' and package.get('repository') == 'https://github.com/' + REPOSITORY,
            'The public crate is not linked to the official SDK repository.')
    owner = os.environ.get('CRATES_IO_OWNER', '')
    require(re.fullmatch(r'[A-Za-z0-9](?:[A-Za-z0-9-]{0,38})', owner), 'The reviewed crate owner is missing.')
    owners = public_json('https://crates.io/api/v1/crates/raims/owners').get('users', [])
    require(any(item.get('login') == owner for item in owners), 'The reviewed account does not own the public crate.')
    require(public_download('https://static.crates.io/crates/raims/raims-' + version + '.crate') == raw,
            'The public crate bytes differ from the canonical validation artifact.')
    request = urllib.request.Request('https://index.crates.io/ra/im/raims', headers={
        'User-Agent': 'Raims-Rust-Release', 'Cache-Control': 'no-cache'})
    try:
        lines = read_response(request).decode('utf-8').splitlines()
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        raise RuntimeError('The public sparse registry index could not be read.') from None
    rows = [json.loads(line) for line in lines]
    found = [item for item in rows if item.get('vers') == version]
    if not found:
        return None
    require(len(found) == 1 and found[0].get('name') == 'raims' and found[0].get('cksum') == digest(raw)
            and found[0].get('yanked') is False, 'The sparse registry version checksum differs.')
    return metadata


def validate_local(artifact, files):
    require(artifact.is_dir() and not artifact.is_symlink(), 'The approved upload directory is missing.')
    paths = list(artifact.rglob('*'))
    require({path.relative_to(artifact).as_posix() for path in paths}
            == {'dist', 'dist/raims.crate', 'dist/publish-metadata.json', 'release-manifest.json'}
            and all(not path.is_symlink() for path in paths), 'The upload directory contains unexpected entries.')
    for name, raw in files.items():
        path = artifact / name if name == 'release-manifest.json' else artifact / 'dist' / name
        require(path.read_bytes() == raw, 'The local crate upload differs from approved CI bytes.')


def permanent_api_key():
    token = os.environ.get('CARGO_REGISTRY_TOKEN', '')
    require(16 <= len(token) <= 8192 and token == token.strip()
            and all(32 <= ord(character) < 127 for character in token),
            'The configured permanent crates.io API token is missing or invalid.')
    return token


def verify_credential():
    # Scoped crates.io publishing tokens cannot introspect /me or token metadata.
    # Account verification and the non-expiring exact-name scopes are reviewed
    # when the environment secret is provisioned. Do not broaden or probe-write.
    token = permanent_api_key()
    print('::add-mask::' + token.replace('%', '%25'), flush=True)
    require(os.environ.get('CRATES_IO_TOKEN_POLICY') == 'raims:publish-new,publish-update:never',
            'The reviewed permanent, exact-crate publishing policy is missing.')
    owner = os.environ.get('CRATES_IO_OWNER', '')
    require(re.fullmatch(r'[A-Za-z0-9](?:[A-Za-z0-9-]{0,38})', owner), 'The reviewed crate owner is missing.')
    print('The configured publishing credential is present; crates.io verifies its authorization on the genuine upload.', flush=True)


def publication_payload(metadata, raw):
    require(0 < len(metadata) <= MAX_PUBLISH_METADATA and 0 < len(raw) <= MAX_CRATE,
            'The registry upload exceeds the reviewed size limits.')
    return struct.pack('<I', len(metadata)) + metadata + struct.pack('<I', len(raw)) + raw


def publish(_artifact, value, files):
    raw = files['raims.crate']
    if public_version(value['version'], raw):
        confirm(value['version'], files)
        print('The exact approved crate is already published; no upload is required.', flush=True)
        return
    verify_credential()
    request = urllib.request.Request('https://crates.io/api/v1/crates/new',
        data=publication_payload(files['publish-metadata.json'], raw), method='PUT',
        headers={'Authorization': permanent_api_key(), 'Content-Type': 'application/octet-stream',
                 'Accept': 'application/json', 'User-Agent': 'Raims-Rust-Release (https://github.com/' + REPOSITORY + ')'})
    try:
        response = json.loads(read_response(request))
        require(isinstance(response, dict) and not response.get('errors')
                and response.get('crate', {}).get('name') == 'raims'
                and response.get('warnings') == {'invalid_categories': [], 'invalid_badges': [], 'other': []},
                'crates.io did not acknowledge the upload; inspect the registry before retrying.')
    except (OSError, RuntimeError, ValueError):
        # No retries of an uncertain write, including a 2xx body with errors.
        if public_version(value['version'], raw):
            confirm(value['version'], files)
            return
        raise RuntimeError('The crate upload was not confirmed; inspect the registry before retrying.') from None
    confirm(value['version'], files)


def confirm(version, files, wait_seconds=600):
    deadline = time.monotonic() + wait_seconds
    while True:
        metadata = indexed(version, files['raims.crate'])
        if metadata:
            print('crates.io and the sparse registry serve the exact approved crate bytes.', flush=True)
            return
        require(time.monotonic() < deadline, 'The registry has not confirmed the crate bytes; inspect before retrying.')
        time.sleep(min(10, max(0, deadline - time.monotonic())))


def stage(source, output):
    """Build public provenance from a passed current-toolchain canonical report."""
    report = json.loads((output / 'report.json').read_text(encoding='utf-8'))
    require(report.get('status') == 'passed' and report.get('sourceUnchanged') is True
            and report.get('physicalTestsOnlyInTmp') is True and report.get('rustProfileUnchanged') is True
            and report.get('sourceAndPackageConsumers') is True and report.get('temporaryProcessesTerminated') is True
            and report.get('liveApiCalled') is False
            and report.get('dependencyAudit', {}).get('status') == 'passed'
            and report.get('rustc', '').startswith('rustc 1.99.1 '), 'The complete current Rust canonical gate did not pass.')
    head = git(source, 'rev-parse', 'HEAD')
    require(SHA.fullmatch(head) and head == os.environ.get('GITHUB_SHA')
            and not git(source, 'status', '--porcelain', '--untracked-files=all'), 'The canonical source identity differs.')
    version = source_version(source)
    package = report['package']
    raw = Path(package['path']).read_bytes()
    require(digest(raw) == package['sha256'], 'The approved crate bytes changed.')
    metadata = metadata_bytes(raw, {'version': version, 'sourceRevision': head})
    toolkit = os.environ.get('RAIMS_RUST_TOOLKIT_REF', '')
    require(SHA.fullmatch(toolkit) and toolkit != '0' * 40, 'The reviewed tooling commit is missing.')
    dist = output / 'dist'
    require(not dist.exists(), 'Use a fresh distribution output.')
    dist.mkdir()
    files = {'raims.crate': raw, 'publish-metadata.json': metadata}
    for name, value in files.items():
        (dist / name).write_bytes(value)
    manifest = {'package': {'name': 'raims', 'version': version}, 'sourceRevision': head,
                'toolkitRevision': toolkit, 'runtimeVersions': RUNTIME_VERSIONS,
                'files': {name: digest(value) for name, value in files.items()}}
    (output / 'release-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=('stage', 'prepare', 'verify', 'credential', 'publish', 'confirm'))
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--artifact', type=Path, required=True)
    args = parser.parse_args()
    source, artifact = args.source.resolve(), args.artifact.resolve()
    workspace = Path(os.environ.get('GITHUB_WORKSPACE', '.')).resolve()
    require(workspace / '.tmp' in artifact.parents, 'Release artifacts must stay in workspace .tmp.')
    if args.command == 'stage':
        stage(source, artifact)
        return 0
    value = context(source)
    proven = validation(value['sourceRevision'])
    files, _identifier = artifact_files(value, proven)
    release_files(value, files, create_missing=args.command == 'prepare')
    if args.command == 'prepare':
        require(not artifact.exists(), 'Use a new upload directory.')
        (artifact / 'dist').mkdir(parents=True)
        for name, raw in files.items():
            (artifact / name if name == 'release-manifest.json' else artifact / 'dist' / name).write_bytes(raw)
    else:
        validate_local(artifact, files)
    if args.command == 'verify':
        required = not public_version(value['version'], files['raims.crate'])
        if os.environ.get('GITHUB_OUTPUT'):
            with Path(os.environ['GITHUB_OUTPUT']).open('a', encoding='utf-8') as output:
                output.write('publishRequired=' + ('true' if required else 'false') + '\n')
    elif args.command == 'credential':
        verify_credential()
    elif args.command == 'publish':
        publish(artifact, value, files)
    elif args.command == 'confirm':
        confirm(value['version'], files)
    return 0

if __name__ == '__main__':
    try:
        sys.exit(main())
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError, zipfile.BadZipFile, tarfile.TarError) as error:
        # Never include response bodies or provider exceptions that may echo auth.
        message = str(error) if isinstance(error, RuntimeError) else type(error).__name__
        print('Rust release verification failed: ' + message, file=sys.stderr)
        sys.exit(1)
