"""Adds or removes the WHEEL tab in the game's settings data.

The game's Settings menu is built from r6\\config\\settings\\platform\\pc\\options.json. This inserts one
group and its options without changing any existing line, after keeping a copy of the original.
Removing restores that copy, and only if the file is the one this script wrote.

    python scripts/settings_tab.py add "<game folder>" "<backup folder>"
    python scripts/settings_tab.py remove "<game folder>" "<backup folder>"
"""

import hashlib
import json
import os
import shutil
import sys

RELATIVE = os.path.join('r6', 'config', 'settings', 'platform', 'pc', 'options.json')
BACKUP = 'options-pc-original.json'
MARK = '/wheel'
GROUP = '/wheel/driving'
GROUPS = [
    {'group_name': MARK, 'display_name': 'WHEEL', 'order': 10},
    {'group_name': GROUP, 'display_name': 'Driving', 'order': 0},
]


def option(name, label, description, order, kind, default, low, high, step):
    return {
        'name': name,
        'display_name': label,
        'description': description,
        'order': order,
        'is_visible': True,
        'in_pre_game': True,
        'in_game': True,
        'update_policy': 'immediately',
        'type': kind,
        'value': default,
        'default_value': default,
        'min_value': low,
        'max_value': high,
        'step_value': step,
    }


OPTIONS = [{
    'group_name': GROUP,
    'options': [
        option('SteeringRange', 'Steering range', 'Degrees of wheel turn from full left to full right. '
               'Assumes G HUB is set to 900.', 0, 'int', 360, 180, 900, 30),
        option('CenterResponse', 'Center response', 'Higher values soften steering near center.',
               1, 'float', 1.0, 1.0, 4.0, 0.1),
        option('SteeringSpeed', 'Steering speed', 'How quickly the tires follow the wheel. '
               'Applies at the next game start.', 2, 'int', 2, 1, 10, 1),
        option('BoostStrength', 'Boost strength', 'How far boost can raise the speed, as a percentage.',
               3, 'int', 25, 25, 200, 5),
    ],
}]


def sha256(path):
    with open(path, 'rb') as handle:
        return hashlib.sha256(handle.read()).hexdigest()


def block(entries, newline):
    """Entries as text, one level inside an array, each followed by a comma."""
    out = []
    for entry in entries:
        text = json.dumps(entry, indent=4)
        out.append('\n'.join('        ' + line for line in text.splitlines()) + ',')
    return newline.join(out) + newline


def with_tab(text):
    newline = '\r\n' if '\r\n' in text else '\n'
    for key, entries in (('"groups": [', GROUPS), ('"options": [', OPTIONS)):
        start = text.find(key + newline)
        if start < 0:
            raise SystemExit(f'Unexpected file layout: {key} not found.')
        at = start + len(key + newline)
        text = text[:at] + block(entries, newline) + text[at:]
    return text


def add(game, backup_folder):
    target = os.path.join(game, RELATIVE)
    with open(target, encoding='utf-8', newline='') as handle:
        original = handle.read()
    if json.dumps(MARK) in original:
        raise SystemExit('The tab is already there.')
    os.makedirs(backup_folder, exist_ok=True)
    backup = os.path.join(backup_folder, BACKUP)
    if not os.path.exists(backup):
        shutil.copyfile(target, backup)
    changed = with_tab(original)
    data = json.loads(changed)
    groups = [group['group_name'] for group in data['groups']]
    if groups[:2] != [MARK, GROUP] or len(data['groups']) != len(json.loads(original)['groups']) + 2:
        raise SystemExit('The change did not parse as expected, so nothing was written.')
    with open(target, 'w', encoding='utf-8', newline='') as handle:
        handle.write(changed)
    with open(os.path.join(backup_folder, 'options-pc-with-tab.sha256'), 'w', encoding='ascii') as handle:
        handle.write(sha256(target) + '\n')
    print('original', sha256(backup))
    print('with tab', sha256(target))


def remove(game, backup_folder):
    target = os.path.join(game, RELATIVE)
    backup = os.path.join(backup_folder, BACKUP)
    with open(os.path.join(backup_folder, 'options-pc-with-tab.sha256'), encoding='ascii') as handle:
        expected = handle.read().strip()
    if sha256(target) != expected:
        raise SystemExit('The file has changed since the tab was added, so it was left alone.')
    shutil.copyfile(backup, target)
    print('restored', sha256(target))


if __name__ == '__main__':
    if len(sys.argv) != 4 or sys.argv[1] not in ('add', 'remove'):
        raise SystemExit(__doc__)
    {'add': add, 'remove': remove}[sys.argv[1]](sys.argv[2], sys.argv[3])
