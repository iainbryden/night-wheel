# Test P1-READ: read-only device read and automated checks

Date: 2026-11-04
Result: PASS for the checks listed; physical control mapping NOT RUN
Evidence type: AUTO, DEVICE (software reading only)
Source checkpoint and build/package hashes: uncommitted working tree; debug `wd_diag.exe` SHA-256 `bd911f77dcf394db9f42aa8dde46a6cb13df7798e7d14b8157fe694b5c97f54d`; `Cargo.lock` SHA-256 `cdfd9962f6ba86d34f300077c52235f11457a041e6fe1565fba79ea1820ad8cf`
Game/Windows/dependency versions: no game; Windows 26H2 build 26300.9818; Rust 1.99.1; `windows` 0.62.2
Device/driver/firmware: `VID_046D PID_C24F` rev 8900; G HUB driver 2024.1.0.0; firmware not read
Setup/profile: wheel connected; nobody confirmed whether it was being touched.

Steps:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
target\debug\wd_diag.exe --list
target\debug\wd_diag.exe --plain --seconds 5
```

Expected: checks clean, tests pass, and the wheel is listed and reports are decoded without sending anything to it.

Actual:

- Format and lint checks were clean.
- 48 of 48 tests passed: 44 in `wd_core`, 4 in `wd_diag`.
- The source references no device-write API (searched for `WriteFile`, `CreateFile`, `DeviceIoControl`, `HidD_Set`, `IDirectInput`, `ForceFeedback`).
- Listing is in [P1-device-list.txt](../../evidence/P1-device-list.txt). The device reports an output report of 17 bytes, which this program never uses.
- A 5-second live run decoded 150 reports:

| Control | Seen | Reported range |
|---|---|---|
| Hat | 8 | 0..=7 |
| X | 33057..=33298 | 0..=65535 |
| Y | 255 | 0..=255 |
| Rz | 255 | 0..=255 |
| Z | 255 | 0..=255 |
| FF00:0001 (two entries) | 128 | 0..=255 |

Evidence files: `evidence/P1-device-list.txt`. The live run's output was reviewed in the session and is summarized above, not saved.

Limitations:

- Which physical control maps to which report is **not established**. X, Y, Rz and Z behave as a plausible wheel and three resting pedals, but that is an inference until a person moves each one.
- No button was pressed, so button decoding is unproven on the device.
- Disconnect and reconnect were not exercised.
- Hat value 8 is outside its reported range; it is presumably "not pressed", unconfirmed.
- The two `FF00:0001` entries share a name and may decode identically.
- Raw Input details in SRC-020 were not read from documentation; only this run supports them.

Defect and next diagnostic hypothesis, if any:

X fell steadily during the run, moving about 240 counts (about 0.4% of its range) with no known input. Candidates are a hand on the wheel, the wheel settling, or something else applying force, such as G HUB. This program sends nothing, so it is not the cause. Next step: run again with hands clear and G HUB's state noted. Treat as a noise and rest-drift input for deadzone work, and raise it if the wheel visibly moves by itself.
