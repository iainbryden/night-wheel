# Test P5-SETUP: setup app and its logic

Date: 2026-11-04
Result: PASS for automated checks and rendering; interaction on the real wheel NOT RUN
Evidence type: AUTO, DEVICE (detection only)
Source checkpoint and build/package hashes: branch `setup-app`; debug `wd_setup.exe` SHA-256 `06df8cbd7f0b5aa1f9fcd58443be8f60c0863d63d301edad9810a602c4880c51`; `Cargo.lock` SHA-256 `b2290245e20ce51a997fd4b6fb0163c7ffdc6d6be9d097b2c686b7d1ca3e90c3`
Game/Windows/dependency versions: no game; Windows 26H2 build 26300.9818; Rust 1.99.1; `eframe` 0.37.0; `windows` 0.62.2
Device/driver/firmware: `VID_046D PID_C24F` rev 8900; G HUB driver 2024.1.0.0
Setup/profile: rendering checks used a throwaway settings folder. The real `%LOCALAPPDATA%\WheelDrive` was not created.

Steps:

1. `cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo test --locked --workspace`, `cargo build --locked --workspace`.
2. Search the source for device-write and input-injection APIs.
3. Run the app and capture its window on each screen, opened with the debug-only `WD_SETUP_SCREEN` option.
4. Corrupt the settings file with a valid backup beside it, then run the app.

Expected: checks pass, screens render, no output path exists, and a damaged file falls back to the backup with a visible message.

Actual:

- Lints were clean. 87 of 87 tests passed: 83 in `wd_core`, 4 in `wd_diag`.
- No device-write or `SendInput` use was found.
- The app detected the G29 (`0 reports read` with the wheel idle). Device, Calibrate, Steering and pedals, Assignments and Recovery message all rendered correctly.
- The backup loaded, with the message: "Your settings file couldn't be used (setting 'version' has an invalid value), so the backup was loaded."
- `wd_diag` still lists and reads the wheel after its device code moved to `wd_device`. An idle 2-second run produced 0 reports, so the earlier X drift did not recur.

Defect found and fixed: the indicator dots on Assignments rendered as empty boxes because the font lacks those characters. They were replaced by a "Held" label, and the screen was recaptured. The unit tests could not have caught this.

User check, same build: the user reported that the parts of the app that read the wheel work fine. A screenshot from the user showed the G29 detected on the Device screen. The report did not list individual controls, and did not cover calibration, Listen, Save or reconnect.

Evidence files: this record. Screen captures were reviewed in the session and not kept, to keep the repository free of binaries.

Limitations:

- **No clicks were exercised.** Simulated window messages did not operate the app, so buttons, sliders, the wizard run, Listen and Save have not been used in the real app. Their rules are covered by unit tests in `wd_core`; the screen code connecting them is not.
- "Not available yet" was captured but not reviewed in detail.
- Wheel reading rests on the user's report above. Disconnects have not been reported in this app.
- A pedal held before axis detection starts counts as movement when it is released. This is a known limit of detecting axes by movement.
- `MIN_AXIS_MOVEMENT` (10% of range) is provisional until measured on the wheel.
- The `save` path was unit-tested but not used from the app.

Defect and next diagnostic hypothesis, if any: none open.
