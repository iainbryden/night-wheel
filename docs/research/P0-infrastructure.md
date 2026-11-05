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
| Game build | Patch 2.5 on Steam. Exact build number not recorded here | User statement and the cleanup session's report; confirm the build number at P3 |

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
| red4ext-rs | MIT; latest commit found bumps it to 0.11.2 on 2026-10-27; no GitHub releases; needs Rust 1.95+, CMake 3.21+ and libclang | Its README states no loader or game version; here, CMake is 3.20 and libclang was not found. Only an in-game test can show it works |
| REDmod | Official tools, with their own EULA. Documentation (dated 2022) covers scripts, tweaks, archives, sound and animation, and does not mention native code or input devices | Not suitable alone for wheel input |
| Other routes | Compared in [D18](../decisions/D18-game-route.md) | Pause behaviour needs an in-game test |

## Language

See [D16](../decisions/D16-language.md).

## Settings UI during pause

Compared in [D18](../decisions/D18-game-route.md), which is proposed and untested. No document read says whether drawing or input works during pause for any option. It must not rely on a shared settings mod.

CD Projekt Red's 2.5 notes confirm in-game mod browsing and installing through mod.io, and the modding page describes a Mods menu with an enable checkbox. Neither mentions per-mod settings, so no official route for an included settings screen has been found. The RED4ext v1.31.0 notes say it supports that menu together with redscript mods, and give no further detail.

## Next research

1. Read the Microsoft racing wheel guide and DirectInput documentation in full.
2. Find Logitech's official SDK page and licence.
3. Read the Raw Input and HID pages listed as unread in SRC-020.
4. Get the game build from the user, then check loader compatibility.
