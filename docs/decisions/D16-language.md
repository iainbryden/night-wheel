# Decision D16: implementation language and toolchain

Status: accepted by the user on 2026-11-04 for Rust. The game bridge (option B or C) is still open until P3.
Date: 2026-11-04
Requirements affected: all; R19 directly.

Problem: the package leaves the language open. The user asked whether Rust can be used.

Alternatives:

| Option | Benefits | Costs and unknowns |
|---|---|---|
| A. All C++ | Same language as RED4ext.SDK. Compiler already installed. No language bridge | Memory and thread errors are not caught by the compiler, and those are the failures that block release here. The installed compiler is MSVC 2019; whether the SDK builds on it is untested |
| B. All Rust, using red4ext-rs | One language. Compiler-checked ownership suits the output and lifecycle rules. Built-in test runner | Rust is not installed. red4ext-rs has no GitHub releases, so it must be pinned to a commit. Its coverage of the target game build is unverified, and it adds a dependency between the product and the game |
| C. Rust plus a small C++ bridge | Rust for product logic; direct use of the SDK at the game boundary | Two toolchains and a hand-maintained boundary |

Approved source/evidence references: SRC-003, SRC-005, SRC-006, SRC-007; `docs/research/P0-infrastructure.md`.

Choice and rationale (proposed): Rust for all product code, starting with the standalone P1 diagnostic and pure logic. Choose between B and C at P3, when an inert game module has been tried on the real build.

- P1 and P2 do not touch the game, so nothing here commits the project to a game bridge.
- Calibration, mapping, mode and output-ownership logic stay in a crate with no game or device dependency. They survive whichever bridge is chosen.
- Rust only protects product code. Game calls remain `unsafe`, so version gates, lifetime checks and crash testing are needed regardless.
- If neither bridge passes at P3, the fallback is a thin C++ plugin calling the Rust logic through a C interface.

Dependencies/licensing implications: adds DEP-006 and DEP-007 now, and possibly DEP-005 at P3. All are permissively licensed according to the checks so far.

Risks and unknowns:

- Whether the `windows` crate exposes the device interfaces needed. Verify before writing the input layer.
- Whether red4ext-rs supports the user's game build.
- Whether any route can drive the transmission. This is language-independent and remains gated at P6.

Validation and acceptance: P1 builds from a fresh checkout and tests pass; the diagnostic shows real G29 controls; at P3 an inert module loads and unloads cleanly on the verified build.

Reversal/rollback: the logic crate has no game dependency. Moving to option C or to a C++ plugin replaces only the game boundary. Moving wholly to C++ would mean rewriting, which is cheapest now, before P1 code exists.
