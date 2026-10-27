#!/usr/bin/env -S uv run --script
"""Capture native runner toolchain identity before compiler-cache restoration.

Windows includes the installed SDK/toolset inventory conservatively: installing
an additional version invalidates the profile even if CMake selects the old one.
Compiler caches retain their own per-object compiler/input correctness checks.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

import host_platform as host


AXES = {('windows', 'x64'), ('windows', 'arm64'), ('macos', 'x86_64'), ('macos', 'arm64')}


def command(arguments):
    # A file bounds resident memory even if a tool unexpectedly emits a log.
    with tempfile.TemporaryFile() as output:
        subprocess.run(arguments, stdout=output, stderr=subprocess.DEVNULL, check=True, timeout=30)
        output.seek(0)
        value = output.read((64 << 10) + 1)
    if len(value) > 64 << 10:
        raise ValueError('native tool version output budget exceeded')
    return value.decode('utf-8-sig').strip()


def tool_versions(platform, architecture):
    if platform == 'macos':
        if not host.is_macos():
            raise ValueError('macOS identity requires a macOS runner')
        return {'xcode': command(['xcodebuild', '-version']),
                'compiler': command(['xcrun', 'clang++', '--version']),
                'sdk': command(['xcrun', '--sdk', 'macosx', '--show-sdk-version']),
                'sdk_build': command(['xcrun', '--sdk', 'macosx', '--show-sdk-build-version'])}
    if not host.is_windows():
        raise ValueError('Windows identity requires a Windows runner')
    vswhere = Path(os.environ['ProgramFiles(x86)']) / 'Microsoft Visual Studio/Installer/vswhere.exe'
    installations = json.loads(command([str(vswhere), '-latest', '-products', '*', '-requires',
                                        'Microsoft.VisualStudio.Component.VC.Tools.x86.x64', '-format', 'json', '-utf8']))
    if not isinstance(installations, list) or len(installations) != 1:
        raise ValueError('expected one selected Visual Studio installation')
    installation = installations[0]
    root = Path(installation['installationPath'])
    default = (root / 'VC/Auxiliary/Build/Microsoft.VCToolsVersion.default.txt').read_text().strip()
    if not re.fullmatch(r'[0-9]+(?:\.[0-9]+)+', default):
        raise ValueError('invalid Visual Studio default toolset')
    toolsets = root / 'VC/Tools/MSVC'
    import winreg
    with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, r'SOFTWARE\Microsoft\Windows Kits\Installed Roots',
                        access=winreg.KEY_READ | winreg.KEY_WOW64_32KEY) as key:
        sdk_root = Path(winreg.QueryValueEx(key, 'KitsRoot10')[0])
    sdk_versions = sorted(path.name for path in (sdk_root / 'Include').iterdir()
                          if path.is_dir() and re.fullmatch(r'10\.0\.[0-9]+\.[0-9]+', path.name))
    available = sorted(path.name for path in toolsets.iterdir() if path.is_dir())
    if not sdk_versions or default not in available:
        raise ValueError('missing Windows SDK or selected compiler toolset')
    return {'visual_studio': installation['installationVersion'], 'default_tools': default,
            'available_tools': available, 'sdk_versions': sdk_versions,
            'compiler_path': str(toolsets / default / 'bin' / 'Hostx64' / architecture / 'cl.exe')}


def fingerprint(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def capture(platform, architecture, configuration):
    if (platform, architecture) not in AXES or configuration not in ('Release', 'RelWithDebInfo', 'Debug', 'MinSizeRel'):
        raise ValueError('unsupported native cache axis/configuration')
    versions = tool_versions(platform, architecture)
    if platform == 'macos':
        compiler = fingerprint({'xcode': versions['xcode'], 'compiler': versions['compiler']})
        sdk = fingerprint({'version': versions['sdk'], 'build': versions['sdk_build']})
        cache_format = 'xcode-compilation-cas-v1'
    else:
        path = Path(versions['compiler_path'])
        if path.stat().st_size > 128 << 20:
            raise ValueError('compiler binary size budget exceeded')
        checksum = hashlib.sha256()
        with path.open('rb') as stream:
            for chunk in iter(lambda: stream.read(1 << 20), b''):
                checksum.update(chunk)
        compiler = fingerprint({'visual_studio': versions['visual_studio'], 'default': versions['default_tools'],
                                'available': versions['available_tools'], 'cl_sha256': checksum.hexdigest()})
        sdk = fingerprint(versions['sdk_versions'])
        cache_format = 'ccache-4.14.1'
    return {'platform': platform, 'architecture': architecture, 'configuration': configuration,
            'compiler': compiler, 'sdk': sdk, 'cache_format': cache_format}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--platform', choices=('windows', 'macos'), required=True)
    parser.add_argument('--architecture', required=True)
    parser.add_argument('--configuration', required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--expected', type=Path)
    parser.add_argument('--diagnostics', type=Path)
    args = parser.parse_args()
    identity = capture(args.platform, args.architecture, args.configuration)
    # Preserve the captured profile even when a fresh publisher VM has a
    # different image. The mismatch still fails before any transport operation.
    with args.output.open('x') as stream:
        json.dump(identity, stream, sort_keys=True, indent=2)
        stream.write('\n')
    differences = []
    if args.expected is not None:
        with args.expected.open('rb') as stream:
            encoded = stream.read((64 << 10) + 1)
        if len(encoded) > 64 << 10:
            raise ValueError('expected native identity budget exceeded')
        expected = json.loads(encoded)
        if not isinstance(expected, dict) or set(expected) != set(identity):
            differences = ['schema']
        else:
            differences = sorted(field for field in identity if identity[field] != expected[field])
    if args.diagnostics is not None:
        report = {'schema': 1, 'status': 'mismatch' if differences else 'match' if args.expected else 'recorded',
                  'differing_fields': differences,
                  'runner_image_os': os.environ.get('ImageOS', '')[:256],
                  'runner_image_version': os.environ.get('ImageVersion', '')[:256]}
        with args.diagnostics.open('x') as stream:
            json.dump(report, stream, sort_keys=True, indent=2)
            stream.write('\n')
    if differences:
        raise ValueError('native runner identity differs from completed build; fields: ' + ', '.join(differences))


if __name__ == '__main__':
    main()
