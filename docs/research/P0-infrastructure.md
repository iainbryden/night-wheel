# P0 infrastructure research

Status: in progress, 2026-11-04. Nothing here is a verified technical capability unless marked as observed.

## Observed on this PC

| Item | Result | Method |
|---|---|---|
| Wheel | G HUB G29 Driving Force Racing Wheel USB, `VID_046D&PID_C24F`, status OK | Read-only Plug and Play query |
| Wheel driver | 2024.1.0.0, dated 2024-06-18, signed | Read-only driver query |
| Windows | 26H2, build 26300.9818 | Registry read |
| Compiler | MSVC 14.29.30133 (VS 2019 Build Tools), x64 | File check |
| CMake | Bundled with VS 2019 Build Tools; not on PATH | File check |
| Windows SDK | 10.0.19041.0; includes `dinput.h` and `hidusage.h` | File check |
| Rust | 1.99.1, installed by rustup on 2026-11-04 with user approval | Version commands |
| Game build | User states the latest version is 2.5 (2026-11-04). Installed build and store not yet confirmed | User statement; to be confirmed from the install |

The Plug and Play name is what Windows reports. Live controls have not been read yet.

## Wheel access options

| Route | Input | Force feedback | LEDs | Status |
|---|---|---|---|---|
| Windows.Gaming.Input `RacingWheel` | Documented | Documented through `WheelMotor`; supported on selected HID devices | Not documented in what was read | Guide seen through search summary only; G29 behaviour untested |
| DirectInput | To verify | To verify | To verify | Docs not yet opened |
| Raw HID | To verify | Would need protocol knowledge | Would need protocol knowledge | No approved protocol source yet |
| Logitech SDK | To verify | To verify | To verify | Official copy and licence not found yet. Requires review of the licence before any use |

P1 needs input only, with no motor output.

**P1 choice: Windows Raw Input with HID report parsing.** It only receives reports, needs no device-specific knowledge, and worked against the G29 on this PC (see [the test record](../records/P1-device-read.md)). This choice covers input for the diagnostic only. It does not settle force feedback or LEDs, which are compared again at P2.

## Game loading options

| Route | Facts so far | Open |
|---|---|---|
| RED4ext loader | MIT; latest release v1.31.0 on 2026-10-24; its notes set patch 2.5 as the minimum game version | Behaviour on the user's installed build |
| red4ext-rs | MIT; latest commit found bumps it to 0.11.2 on 2026-10-27; no GitHub releases | Whether it supports RED4ext v1.31.0 and patch 2.5 |
| REDmod | Official tools, with their own EULA | Whether it can host native input code at all |
| Other routes | Not yet compared | Script-only and overlay frameworks need comparing for settings during pause |

## Language

See [D16](../decisions/D16-language.md).

## Settings UI during pause

Not researched yet. It must be compared before P5 and must not rely on a shared settings mod.

Patch 2.5 coverage mentions official mod support, and the RED4ext v1.31.0 notes refer to an "in-game mod menu". Read CD Projekt Red's patch notes to see whether this gives an official route for an included settings screen. Not confirmed.

## Next research

1. Read the Microsoft racing wheel guide and DirectInput documentation in full.
2. Find Logitech's official SDK page and licence.
3. Read the Raw Input and HID pages listed as unread in SRC-020.
4. Get the game build from the user, then check loader compatibility.
