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
HOW_TO_BIND = ': hover a row and press a wheel control to set, DELETE to clear'
GROUPS = [
    {'group_name': MARK, 'display_name': 'WHEEL', 'order': 10},
    {'group_name': GROUP, 'display_name': 'Driving' + HOW_TO_BIND, 'order': 0},
    {'group_name': '/wheel/flight', 'display_name': 'Flight' + HOW_TO_BIND, 'order': 1},
    {'group_name': '/wheel/camera', 'display_name': 'Camera' + HOW_TO_BIND, 'order': 2},
    {'group_name': '/wheel/radio', 'display_name': 'Radio' + HOW_TO_BIND, 'order': 3},
    {'group_name': '/wheel/other', 'display_name': 'Other' + HOW_TO_BIND, 'order': 4},
]

# The wheel's controls, in the order the plugin expects. crates/wd_game/src/settings.rs has the same
# list, and a test there checks that the two match.
CONTROLS = [
    'NOT BOUND', 'X', 'SQUARE', 'CIRCLE', 'TRIANGLE',
    'D-PAD UP', 'D-PAD RIGHT', 'D-PAD DOWN', 'D-PAD LEFT',
    'RIGHT PADDLE', 'LEFT PADDLE', 'R2', 'L2', 'R3', 'L3', 'SHARE', 'OPTIONS',
    'PLUS', 'MINUS', 'DIAL RIGHT', 'DIAL LEFT', 'RED ENTER', 'PS',
    'BUTTON 13', 'BUTTON 14', 'BUTTON 15', 'BUTTON 16', 'BUTTON 17', 'BUTTON 18', 'BUTTON 19',
]

BIND_HELP = 'To set this, press a wheel control while hovering here. DELETE clears it.'


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


def binding(key, label, what, order, default='NOT BOUND'):
    return {
        'name': key,
        'display_name': label,
        'description': what + ' ' + BIND_HELP,
        'order': order,
        'is_visible': True,
        'in_pre_game': True,
        'in_game': True,
        'update_policy': 'immediately',
        'type': 'string_list',
        'index': CONTROLS.index(default),
        'default_index': CONTROLS.index(default),
        'values': CONTROLS,
        'display_values': CONTROLS,
    }


OPTIONS = [
    {
        'group_name': GROUP,
        'options': [
            option('SteeringRange', 'Steering range', 'How far the wheel turns, in degrees from full left to '
                   'full right, to reach full steering. Lower values make the car respond to less wheel '
                   'movement. Assumes G HUB is set to 900.', 0, 'int', 360, 180, 900, 30),
            option('CenterResponse', 'Center response', 'How gentle steering is near center. 1.0 is direct. '
                   'Higher values soften small movements and still reach full steering at the end of the '
                   'range.', 1, 'float', 1.0, 1.0, 4.0, 0.1),
            option('SteeringSpeed', 'Steering speed', 'How quickly the tires follow the wheel. Higher values '
                   'feel sharper. Applies the next time the game starts.', 2, 'int', 2, 1, 10, 1),
            binding('handbrake', 'Handbrake', "Applies the game's handbrake, for sharp turns and slides.",
                    3, 'LEFT PADDLE'),
            binding('horn', 'Horn', 'Sounds the horn.', 4),
            binding('lights', 'Lights', "Changes the car's lights.", 5, 'R2'),
            binding('vehicle_exit', 'Exit vehicle', 'Gets out of the car.', 6),
            binding('autodrive', 'Autodrive', "Turns the game's Autodrive on or off. Boost and flight don't "
                    'work while it drives.', 7, 'CIRCLE'),
            binding('combat_mode', 'Combat mode', "Switches to the game's combat mode in the car.", 8, 'SQUARE'),
            binding('boost', 'Boost', 'Hold for a burst of extra speed lasting up to 5 seconds. Letting go or '
                    'touching the brake ends it early. Either way, boost is then unavailable for 3 seconds, '
                    'and the next burst takes a new press. Works on the road and in flight.', 9, 'L3'),
            option('BoostStrength', 'Boost strength', 'How far boost can raise your speed, as a percentage above '
                   'the fastest speed reached in this car without boost. Higher values also push harder and '
                   "raise flight's boosted top speed.", 10, 'int', 25, 25, 200, 5),
        ],
    },
    {
        'group_name': '/wheel/flight',
        'options': [
            binding('fly_up', 'Climb / start flight', 'Press once to lift the car off the ground and fly. Hold '
                    'to climb. In flight, the wheel turns, the throttle speeds up and the brake slows down. '
                    'The car can pass through buildings.', 0, 'PLUS'),
            binding('fly_down', 'Descend / land', 'Hold to go down. Keep holding for half a second at the '
                    'ground to land and return to normal driving.', 1, 'MINUS'),
        ],
    },
    {
        'group_name': '/wheel/camera',
        'options': [
            binding('camera_change', 'Change camera', 'Switches to the next driving camera.', 0, 'D-PAD UP'),
            binding('rear_view', 'Look behind', 'Looks behind the car.', 1, 'D-PAD DOWN'),
            binding('camera_reset', 'Reset camera', 'Puts the camera back behind the car.', 2),
            binding('zoom_in', 'Zoom in', 'Zooms the camera in.', 3, 'D-PAD RIGHT'),
            binding('zoom_out', 'Zoom out', 'Zooms the camera out.', 4, 'D-PAD LEFT'),
        ],
    },
    {
        'group_name': '/wheel/radio',
        'options': [
            binding('radio_menu', 'Radio menu', "Opens the game's radio menu.", 0, 'R3'),
            binding('radio_toggle', 'Radio on/off', 'Turns the car radio on or off.', 1, 'RED ENTER'),
            binding('radio_next', 'Next radio station', 'Tunes to the next station.', 2, 'DIAL RIGHT'),
            binding('radio_previous', 'Previous radio station', 'Tunes to the previous station.', 3, 'DIAL LEFT'),
            binding('radio_volume_up', 'Radio volume up', 'Raises the car radio volume one step.', 4),
            binding('radio_volume_down', 'Radio volume down', 'Lowers the car radio volume one step.', 5),
        ],
    },
    {
        'group_name': '/wheel/other',
        'options': [
            binding('scanner', 'Scanner', "Uses the game's scanner.", 0, 'PS'),
            binding('pause_menu', 'Pause menu', 'Opens the pause menu.', 1, 'OPTIONS'),
            binding('hud_toggle', 'HUD on/off', "Hides or shows the game's HUD while you drive. It comes back "
                    'when you leave the driver seat or load a save.', 2),
        ],
    },
]


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
    if groups[:2] != [MARK, GROUP] or len(data['groups']) != len(json.loads(original)['groups']) + len(GROUPS):
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
