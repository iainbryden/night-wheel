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

Version 0.22.0, user report: "works well." The saved values are range 360, center response 1.0 and speed x2. The slider buttons work and saved each change, per the log. G HUB was at sensitivity 50 and range 900, by a later screenshot, and these are now the defaults.

## WolvenKit read, 2026-11-06

Approved by the user. WolvenKit.Console 9.1.0 came from the project's GitHub releases, and its SHA-256 `7caf49cdf0f478b484b1a5a27e5aef261b879fabea7e72e33ca60eff3b834f4f` matches the digest GitHub publishes. It is GPL-3.0 and isn't shipped with the mod. It's kept in `out/tools/`, outside Git and the game folder. It extracted one file from `basegame_1_engine.archive`, and it changed no game file. See DEP-016.

`settings_main.inkwidget` holds 18 library items, including `settingsSelectorInt`, `settingsSelectorFloat`, `settingsSelectorBool`, `settingsSelectorIntList`, `settingsSelectorStringList`, `settingsSelectorKeyBinding`, `settingsSelectorButton` and `settingsCategory`. The slider row is 1800 by 80: `layout/labels/label` for the name, `layout/container` carrying an `inkSliderController`, `layout/container/slidingArea/knob/txtValue` for the value, and `layout/container/btnLeft` and `btnRight`. Its root carries `SettingsSelectorControllerInt`.

## Test build

Version 0.23.0, not run in the game. The three steering rows spawn `settingsSelectorInt` through the pause menu's controller and use its look and slider, with this script handling the arrows. If the row or any named part is missing, the page draws its own row instead, and the log says why. Only these rows are converted, so the two looks can be compared.

Risks for the test:

- The row's `Setup` is never called, so the row's own controller has no game setting behind it. If its handlers use that when hovered or clicked, they could misbehave, and in the worst case the game could crash. Keep a save first.
- The slider's knob may not draw or move correctly with input turned off.
- The row depends on a game file's internal names, which a patch could change. The fallback covers missing names, not changed behaviour.

## Run of 0.23.0, 2026-11-06

Approved by the user; files replaced and checksums verified. The screenshot shows the old drawing, and the log says `game slider: row has changed` three times. So the row spawned from the pause menu's controller, but looking up one of its parts failed. Which part isn't known, because the check covered them all at once. No crash was reported.

## Page rebuild

Version 0.24.0, not run in the game. The whole page now tries the game's widgets: `settingsCategory` for section headers, `settingsSelectorKeyBinding` for binding rows and `settingsSelectorInt` for sliders. Requested by the user, comparing the page with the game's Sound settings.

- Parts are found one name at a time, and the log names the first one missing.
- Steering comes first in Driving, and Flight moves to the top of the right column to balance the page. Boost strength joins the same slider code.
- Rows are 1800 wide, so the columns take 3680 of the 3840 width.
- An invisible layer over each row takes the mouse, so the game's controllers shouldn't handle it. Those controllers still exist without a game setting behind them, and how they behave at spawn is untested.
- The fallback drawing was rebuilt to match the layout.
- The arrow click areas are placed from the widget file's sizes, and may need adjusting.

## Run of 0.24.0, 2026-11-06

Approved by the user; files replaced and checksums verified. Log kept as `out/game-before/night-wheel-run36.log`. The game's rows and headers now draw, per the user's screenshots. User report:

- The top of the page is cut off.
- The A and D hints appear on hover but don't work, and the row can't be slid with the mouse.
- Long values such as "360 DEGREES" cover the knob and the slider.
- The game's menus have one column and consistent ESC and Restore Defaults prompts. This page has neither.

The log says `game row: slider controller missing` 32 times, so `GetController` on `layout/container` returned nothing that casts to `inkSliderController`, and the knob can't follow the value. The hints showing on hover mean the row's own controllers still respond, despite the layer meant to take the mouse. So only the look is the game's. The behaviour is this mod's, and each missing behaviour would have to be rebuilt by hand.

## The Settings menu is data driven

`r6\config\settings\options.json` (about 95 KB) and `r6\config\settings\platform\pc\options.json` (about 118 KB) define the menu. The PC file has `version` 152, `groups` and `options`, and each option names its group, such as `/audio/misc`, with fields such as `display_name`, `description`, `order`, `is_visible`, `in_pre_game`, `in_game`, `update_policy`, `type`, `default_value` and `value`. The tabs appear to come from the top of each group's name. `SettingsMainGameController` has `PopulateCategories`, `PopulateCategorySettingsOptions` and `PopulateSettingsData`. How the game treats a new group is untested.

This suggests a real WHEEL tab: add a group and options to the data, and let the game draw and run them, including mouse sliding, A and D, ESC and Restore Defaults. The script already reads a game setting this way, for radio volume. The costs:

- It overwrites a base game file. A patch or file check would restore it and remove the tab, and other mods editing that file would conflict. It needs a backup, and uninstall must restore the file.
- The game's rows don't appear able to listen for a wheel press. Bindings might use a list of controls per function, or stay on this page.

A search for an official settings API found only articles from the 2021 launch of the official tools, and nothing on this. Community settings mods exist, but this project doesn't depend on them or read their source.

## WHEEL tab test

Chosen by the user. Version 0.25.0, plus `scripts/settings_tab.py`, which inserts a `/wheel` group at order 10, a `/wheel/driving` section and four options into `r6\config\settings\platform\pc\options.json`, without changing existing lines. It keeps a copy of the original in `out/game-before/`, and removing the tab restores that copy only if the file is unchanged since.

- The tab's values start from the mod's saved values when a session starts, and changes flow back when leaving the menu. So changes made in the main menu before loading a game are overwritten by the saved values. That's a known weakness of this test.
- The labels are plain text rather than the game's localization keys, and how the menu treats that is untested.
- Without the tab, the mod's saved values still apply.
- The settings page keeps its own sliders for now, so the two can be compared.
- This is the first time this project changes a base game file rather than adding files.

### Run of 0.25.0, 2026-11-06

Approved by the user; checksums verified. Log kept as `out/game-before/night-wheel-run37.log`. No new crash report appeared when the log was read, with the game still open.

- **The tab works.** The WHEEL tab appears at the end of the top row and fits, and the game draws and runs its rows. The user reports that A and D and ESC work. The screenshot also shows the DEFAULTS button and the Restore Defaults prompt, not yet reported as tried. Whether the mouse can slide the rows isn't reported either.
- **Values.** The rows showed 360, 1, 2 and 200, the saved values. `UserSettings.json` holds them under `/wheel/driving`, so the game stores these like its own settings.
- **Labels.** Each plain text label is shown with a `<NOT LOCALIZED>` prefix, including the tab name and section title.

### Label cleanup

Version 0.25.1, not run in the game. After the menu builds its tab, section headers and rows, a small function removes that prefix from any text under them. Descriptions show on hover, and this doesn't cover them yet. If a menu fills a label later than these hooks run, the prefix would remain. A proper translation entry would avoid this but needs a further framework or game file.
