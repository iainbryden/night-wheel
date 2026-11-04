# Test P1-FRESH-BUILD: build and test from a fresh clone

Date: 2026-11-04
Result: PASS
Evidence type: AUTO
Source checkpoint and build/package hashes: commit `3a29da4221cdcbec05b4d98da6d468aa6755e18b` (tested as `aac30f85` before the author email was corrected; the file tree is identical); debug `wd_diag.exe` from the clone SHA-256 `9b48a2c401b64a46cc00992be4670ba641f4b3be62f85f4c68c889a4588dc92c`
Game/Windows/dependency versions: no game; Windows 26H2 build 26300.9818; Rust 1.99.1; dependencies locked by `Cargo.lock`
Device/driver/firmware: not relevant, apart from the `--list` smoke run below.
Setup/profile: `git clone` into an empty temporary folder, which was removed afterwards.

Steps: `cargo fmt --all --check`, then `cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo test --locked --workspace`, `cargo build --locked --workspace`, and `wd_diag --list`.

Expected: everything passes from committed files alone, and the clone leaves no changed files.

Actual:

- Format and lint checks were clean.
- 48 of 48 tests passed.
- The build succeeded, and `--list` found the G29.
- `git status` showed no changes after building.

Evidence files: this record.

Limitations:

- The toolchain and crate cache already existed on this PC, so this does not prove a build on a clean machine.
- The binary hash differs from the one in `P1-device-read.md` because the build path differs. Reproducible binaries have not been attempted.
- No hosted CI exists, and the CI secret, path and archive checks in `BUILD_AND_DEPENDENCIES.md` are not implemented.

Defect and next diagnostic hypothesis, if any: none.
