# AGENTS.md — pheno-cdylib-bridge

## Scope

A thin Rust crate that compiles to a `cdylib` and exposes a C-ABI surface over the pheno-* memory substrate (and, in future versions, config / port-adapter / flags / errors). The primary consumer is the upstream `antinomyhq/forgecode` Go process; secondary consumers are any FFI host (Python via cffi, Swift via bridging header, etc.).

## Quality bar

Per AGENTS.md ADR-040 (test coverage gates per tier) and ADR-042B (substrate quality bar):

- 80% line coverage on `src/lib.rs` (tier: lib).
- All entry points have a smoke test (Rust side via `tests/ffi_smoke.rs`, C side via `c/examples/smoke.c`).
- `bash -n` clean on every shell script.
- `cargo build --release` produces both `libpheno_bridge.{so,dylib}` and `libpheno_bridge.a`.

## Branch / commit / PR

- Branch: `feat/<req-id>-<slug>-<date>` per fleet convention.
- Commits: Conventional Commits.
- PR labels: `governance` for cleanup, `L<n>-#<n>` for tracking.

## Workspace

This is a single-crate repo. The `thegent-memory` dependency is referenced by relative path (`../../thegent-pr2-v2/crates/thegent-memory`) so PRs can land independently; in a release build, replace with a published version.

## Device

This crate builds in <2 minutes on the MacBook. No `device: heavy-runner` work required.

## Refs

- ADR-096 — forgecode improvement decision (locked 2026-06-23).
- `findings/2026-06-23-forgecode-improvement-plan.md` — master 3-PR plan.
- `thegent/docs/specs/cdylib-bridge/v1.md` — canonical SPEC.
- `thegent/docs/specs/memory/v2.md` — the underlying memory port spec.