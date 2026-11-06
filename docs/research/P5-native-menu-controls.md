# Using the game's own menu controls

Date: 2026-11-06. Asked by the user, because the WHEEL SETTINGS page doesn't look like the game's other menus. Nothing here has been built or run in the game.

Method: searching the unmodified game's script file for names, compiling small tests against a copy of it, and listing the game's config folder. No other mod was consulted. A compile only proves a name exists, not how it behaves.

## What was found

| Item | Result |
|---|---|
| `base\gameplay\gui\fullscreen\settings\settings_main.inkwidget` | Named in the game's scripts. It's the likely home of the settings menu's widgets |
| `SpawnFromExternal(parent, path, name)` | Compiles on game and logic controllers, with that path. A logic controller can call it, so it may work from the pause menu. The widget names inside the file aren't known |
| `SettingsSelectorControllerInt`, `Float`, `Bool`, `List` and others | Exist. `Setup(ConfigVar, Bool)` compiles, so they appear tied to a real game setting |
| `inkSliderController` | Exists, and `Setup(min, max, value, step)` and `GetCurrentValue` compile, so it doesn't look tied to a game setting |
| `r6\config\settings\platform\pc\options.json` | Exists, about 118 KB, apparently the data behind the Settings menu |

## Routes

1. **Spawn the game's own row widgets on this page (recommended to try).** It would look the same as the game's menus, and the page's logic stays this mod's own. The widget names can only come from reading inside `settings_main.inkwidget`, which needs WolvenKit, and that needs the user's approval. If a game patch renames a widget, the page must fall back to its current drawing.
2. **Add entries to the game's own Settings menu by changing `options.json`.** It would be fully native for sliders, but it means overwriting a base game file that other mods and patches may also change. Bindings need the one-press listening this page already has, which a game setting wouldn't obviously provide. Not recommended.
3. **Restyle the current page by hand.** It needs no new tool and adds no dependency on game files, but it can only approximate the look.

## Open question

Whether spawned widgets draw correctly on the page at `inkMenuLayer`, and whether the settings menu's widgets need its controller to work, can only be tested in the game.

## Steering sliders

Version 0.22.0, user report: "works well." The saved values are range 360, center response 1.0 and speed x2. The slider buttons work and saved each change, per the log. Which G HUB values were in use isn't confirmed yet, and the defaults stay neutral until it is.

## WolvenKit read, 2026-11-06

Approved by the user. WolvenKit.Console 9.1.0 came from the project's GitHub releases, and its SHA-256 `7caf49cdf0f478b484b1a5a27e5aef261b879fabea7e72e33ca60eff3b834f4f` matches the digest GitHub publishes. It is GPL-3.0 and isn't shipped with the mod. It's kept in `out/tools/`, outside Git and the game folder. It extracted one file from `basegame_1_engine.archive`, and it changed no game file. See DEP-016.

`settings_main.inkwidget` holds 18 library items, including `settingsSelectorInt`, `settingsSelectorFloat`, `settingsSelectorBool`, `settingsSelectorIntList`, `settingsSelectorStringList`, `settingsSelectorKeyBinding`, `settingsSelectorButton` and `settingsCategory`. The slider row is 1800 by 80: `layout/labels/label` for the name, `layout/container` carrying an `inkSliderController`, `layout/container/slidingArea/knob/txtValue` for the value, and `layout/container/btnLeft` and `btnRight`. Its root carries `SettingsSelectorControllerInt`.

## Test build

Version 0.23.0, not run in the game. The three steering rows spawn `settingsSelectorInt` through the pause menu's controller and use its look and slider, with this script handling the arrows. If the row or any named part is missing, the page draws its own row instead, and the log says why. Only these rows are converted, so the two looks can be compared.

Risks for the test:

- The row's `Setup` is never called, so the row's own controller has no game setting behind it. If its handlers use that when hovered or clicked, they could misbehave, and in the worst case the game could crash. Keep a save first.
- The slider's knob may not draw or move correctly with input turned off.
- The row depends on a game file's internal names, which a patch could change. The fallback covers missing names, not changed behaviour.
