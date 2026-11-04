# Decision D17: separate setup app, built with egui

Status: accepted
Date: 2026-11-04
Requirements affected: R07, R08, R12.

Problem: players need a way to calibrate and assign controls, and in-game UI routes are unproven until the game experiments at P3.

Alternatives:

| Option | Benefits | Costs |
|---|---|---|
| Plain Win32 controls | No new dependencies | Far more code for live bars, sliders and layout |
| Terminal screen | No new dependencies | Poor fit for sliders; not a finished setup experience |
| `egui`/`eframe` | Small amount of UI code; Rust-native; could also serve an overlay later | About 90 more crates to review and keep notices for |

Approved source/evidence references: user decision on 2026-11-04 to build the setup app first; SRC-023; DEP-008.

Choice and rationale: a separate Windows app using `eframe` 0.37.0, with only the `glow` and `default_fonts` features. Rules stay in `wd_core`, so the same settings can serve an in-game screen later.

Dependencies/licensing implications: 111 third-party crates in this build, all under permissive licenses per crate metadata. `self_cell` offers Apache-2.0 or GPL-2.0-only, and Apache-2.0 is the choice here. `epaint_default_fonts` includes OFL-1.1 and Ubuntu Font Licence fonts. Notices are required before any binary is distributed.

Risks and unknowns:

- This does not meet R12's requirement for settings during game pause. An in-game screen is still needed.
- Accessibility support (`accesskit`) is switched off to reduce dependencies. Review before release.
- The app has only one profile file. Named profiles and per-vehicle settings are not built.

Validation and acceptance: every screen rendered; a full wizard and Listen run on the real wheel (P5-SETUP-DEVICE).

Reversal/rollback: `crates/wd_setup` holds the only UI code and can be replaced without touching the logic or device crates.
