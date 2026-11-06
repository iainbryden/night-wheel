"""Writes THIRD_PARTY_NOTICES.md from the locked dependencies of what the package ships."""

import json
import os
import re
import subprocess

# These declare no license in their package data. The values come from reading their LICENSE files.
LICENSE_FILES = {'const-combine': 'MIT', 'red4ext-rs': 'MIT'}
SHIPPED = {'wd_game', 'wd_setup'}
OWN = {'wd_core', 'wd_device', 'wd_game', 'wd_setup', 'wd_diag'}
LICENSE_NAME = re.compile(r'^(licen[cs]e|copying|notice|unlicense|ofl|ufl|copyright)', re.IGNORECASE)
SKIP_FOLDERS = {'target', '.git', 'tests', 'benches', 'examples'}
# These source packages hold no license file, so the texts come from each project's repository.
TEXT_FOLDER = os.path.join('scripts', 'license-texts')
EGUI = ['egui-MIT.txt', 'egui-APACHE.txt']
KEPT_TEXTS = {
    'accesskit': ['accesskit-MIT.txt', 'accesskit-APACHE.txt', 'accesskit-CHROMIUM.txt'],
    'ecolor': EGUI, 'eframe': EGUI, 'egui': EGUI, 'egui-winit': EGUI,
    'egui_glow': EGUI, 'emath': EGUI, 'epaint': EGUI,
    'profiling': ['profiling-MIT.txt', 'profiling-APACHE.txt'],
}

metadata = json.loads(subprocess.run(
    ['cargo', 'metadata', '--locked', '--format-version', '1',
     '--filter-platform', 'x86_64-pc-windows-msvc'],
    capture_output=True, text=True, check=True).stdout)

packages = {package['id']: package for package in metadata['packages']}
nodes = {node['id']: node for node in metadata['resolve']['nodes']}

# Follows normal dependencies only: build tools and test libraries aren't shipped.
pending = [id for id, package in packages.items() if package['name'] in SHIPPED]
seen = set()
while pending:
    current = pending.pop()
    if current in seen:
        continue
    seen.add(current)
    for dependency in nodes[current]['deps']:
        if any(kind['kind'] is None for kind in dependency['dep_kinds']):
            pending.append(dependency['pkg'])

shipped = sorted((packages[id] for id in seen if packages[id]['name'] not in OWN),
                 key=lambda package: (package['name'], package['version']))

rows = [(package['name'], package['version'],
         package['license'] or LICENSE_FILES.get(package['name'], 'Not stated'),
         package['repository'] or '')
        for package in shipped]


def license_texts(package):
    """Every license file in a package's own source, as it was downloaded."""
    root = os.path.dirname(package['manifest_path'])
    found = []
    for name in KEPT_TEXTS.get(package['name'], []):
        with open(os.path.join(TEXT_FOLDER, name), encoding='utf-8') as handle:
            found.append(handle.read().replace('\r\n', '\n').strip())
    for folder, subfolders, files in os.walk(root):
        subfolders[:] = sorted(name for name in subfolders if name not in SKIP_FOLDERS)
        for name in sorted(files):
            if LICENSE_NAME.match(name) and not name.endswith(('.rs', '.toml', '.py')):
                with open(os.path.join(folder, name), encoding='utf-8', errors='replace') as handle:
                    found.append(handle.read().replace('\r\n', '\n').strip())
    return found


# Identical texts appear once, under every library that uses them.
users_of_text = {}
without_text = []
for package in shipped:
    texts = license_texts(package)
    if not texts:
        without_text.append(f"{package['name']} {package['version']}")
    for text in texts:
        users_of_text.setdefault(text, []).append(f"{package['name']} {package['version']}")

with open('THIRD_PARTY_NOTICES.md', 'w', encoding='utf-8', newline='\n') as out:
    out.write('# Third-party notices\n\n')
    out.write('Rust libraries built into `NightWheel.dll` and `NightWheelSetup.exe`, with the license '
              'texts from their own source packages. '
              'This file comes from `scripts/third_party_list.py`.\n\n')
    out.write('RED4ext, redscript, Codeware and TweakXL are separate downloads and aren\'t included.\n\n')
    out.write('The Rust standard library is also built in, under MIT OR Apache-2.0.\n\n')
    out.write('| Library | Version | License | Source |\n|---|---|---|---|\n')
    for name, version, license_name, source in rows:
        out.write(f'| {name} | {version} | {license_name} | {source} |\n')
    out.write('\n## License texts\n')
    for text, users in sorted(users_of_text.items(), key=lambda item: item[1][0]):
        out.write('\n### ' + ', '.join(users) + '\n\n')
        out.write('```text\n' + text.replace('```', "'''") + '\n```\n')

print(len(rows), 'libraries,', len(users_of_text), 'distinct texts')
print('Not stated:', [row[0] for row in rows if row[2] == 'Not stated'])
print('No license file in the source:', without_text)
