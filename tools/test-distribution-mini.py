#!/usr/bin/env python3
"""Check packaged Mini adapters with private peers/settings, never the active desktop."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

REPO = Path(__file__).resolve().parents[1]


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('distribution', type=Path)
    parser.add_argument('--gjs', default=shutil.which('gjs'), help='GJS executable or an explicit private-runtime wrapper')
    parser.add_argument('--plasma-native', action='store_true', help='Also load the packaged applet in a private Plasma test host')
    args = parser.parse_args()
    if not args.gjs:
        parser.error('GJS is required; pass --gjs or install the CI dependencies')
    gjs = str(Path(args.gjs).resolve(strict=True))
    distribution = args.distribution.resolve(strict=True)
    packages = list(distribution.glob('*.pkg.tar.zst'))
    if len(packages) != 1:
        parser.error('Expected exactly one built Arch package')
    root = Path(tempfile.mkdtemp(prefix='llcontrol-mini-distribution-'))
    prefix = root / 'installed'
    prefix.mkdir()
    subprocess.run(['bsdtar', '-xf', str(packages[0]), '-C', str(prefix)], check=True)
    manifest = json.loads((prefix / 'usr/share/doc/ll-control/manifest.json').read_text())
    if manifest != json.loads((distribution / 'manifest.json').read_text()):
        raise ValueError('Embedded package manifest differs')
    for name, info in manifest['files'].items():
        path = prefix / name
        if digest(path) != info['sha256'] or oct(path.stat().st_mode & 0o777) != info['mode']:
            raise ValueError('Package input mismatch: ' + name)
    gnome = prefix / 'usr/share/gnome-shell/extensions/llcontrol-mini@inkyquill.net'
    plasma = prefix / 'usr/share/plasma/plasmoids/net.inkyquill.llcontrol.mini'
    cli = prefix / 'usr/bin/llcontrol'
    assert (gnome / 'mini.mjs').read_bytes() == (plasma / 'contents/code/mini.mjs').read_bytes()
    build = root / 'plasma-build'
    stages = [
        ('cmake', ['cmake', '-S', str(REPO / 'integrations/plasma'), '-B', str(build), '-DCMAKE_BUILD_TYPE=Debug']),
        ('build', ['cmake', '--build', str(build), '-j2']),
        ('ctest', ['ctest', '--test-dir', str(build), '--output-on-failure']),
        ('packaged-model', ['node', '--test', str(REPO / 'integrations/common/tests/mini.test.mjs')]),
        ('gnome-feed', ['python3', str(REPO / 'integrations/gnome/tests/run-feed.py'), str(gnome), '--gjs', gjs]),
        ('gnome-ipc', ['python3', str(REPO / 'integrations/gnome/tests/run-ipc.py'), str(gnome), '--gjs', gjs, '--cli', str(cli)]),
        ('gnome-preferences', ['python3', str(REPO / 'integrations/gnome/tests/run-preferences.py'), str(gnome), '--gjs', gjs]),
    ]
    if args.plasma_native:
        stages.append(('plasma-native', ['python3', str(REPO / 'integrations/plasma/tests/native-plasma.py'), str(build), '--package-root', str(prefix), '--cli', str(cli)]))
    results = []
    print('Mini artifacts:', root, flush=True)
    for name, command in stages:
        env = dict(os.environ)
        if name == 'packaged-model':
            env['LLCONTROL_MINI_MODEL'] = str(gnome / 'mini.mjs')
        log = root / (name + '.log')
        with log.open('w') as stream:
            result = subprocess.run(command, cwd=REPO, env=env, stdout=stream, stderr=subprocess.STDOUT, timeout=180)
        results.append(dict(name=name, command=command, returncode=result.returncode, log=str(log)))
        (root / 'stages.json').write_text(json.dumps(results, indent=2) + '\n')
        print(name, result.returncode, flush=True)
        if result.returncode:
            print(log.read_text(errors='replace')[-12000:], flush=True)
            raise SystemExit(result.returncode)
        if name == 'build':
            assert (build / 'common/mini.mjs').read_bytes() == (gnome / 'mini.mjs').read_bytes(), 'Build/source model differs from package'
    for name, info in manifest['files'].items():
        assert digest(prefix / name) == info['sha256'], 'Test modified package input: ' + name
    (root / 'acceptance.json').write_text(json.dumps(dict(ok=True, distribution=str(distribution),
        package_sha256=digest(packages[0]), cli_sha256=digest(cli), model_sha256=digest(gnome / 'mini.mjs'),
        package_inputs_unchanged=True, stages=results, plasma_native=args.plasma_native,
        real_gnome_shell=False, physical_hardware=False, active_session_installation=False), indent=2) + '\n')
    print('PASS packaged Mini model/Qt/GJS/preferences/read-only CLI' + ('/Plasma host' if args.plasma_native else ''), root)


if __name__ == '__main__':
    main()
