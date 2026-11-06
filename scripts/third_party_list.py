"""Writes THIRD_PARTY_NOTICES.md from the locked dependencies of what the package ships."""

import json
import subprocess

SHIPPED = {'wd_game', 'wd_setup'}
OWN = {'wd_core', 'wd_device', 'wd_game', 'wd_setup', 'wd_diag'}

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

rows = sorted(
    (packages[id]['name'], packages[id]['version'],
     packages[id]['license'] or 'Not stated', packages[id]['repository'] or '')
    for id in seen if packages[id]['name'] not in OWN)

with open('THIRD_PARTY_NOTICES.md', 'w', encoding='utf-8', newline='\n') as out:
    out.write('# Third-party notices\n\n')
    out.write('Rust libraries built into `NightWheel.dll` and `NightWheelSetup.exe`. '
              'This file comes from `scripts/third_party_list.py`. '
              'It lists the licenses each library declares, and the full license texts '
              'are still to be added before any release.\n\n')
    out.write('RED4ext, redscript, Codeware and TweakXL are separate downloads and aren\'t included.\n\n')
    out.write('| Library | Version | License | Source |\n|---|---|---|---|\n')
    for name, version, license_name, source in rows:
        out.write(f'| {name} | {version} | {license_name} | {source} |\n')
print(len(rows), 'libraries')
print('Not stated:', [row[0] for row in rows if row[2] == 'Not stated'])
