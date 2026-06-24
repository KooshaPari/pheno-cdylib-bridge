# Changelog

All notable changes to `pheno-cdylib-bridge` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-06-23

### Added

- Initial scaffold.
- C-ABI surface: `pheno_bridge_version`, `pheno_last_error`, `pheno_string_free`,
  `pheno_memory_new`, `pheno_memory_store`, `pheno_memory_recall`,
  `pheno_memory_forget`, `pheno_memory_free`.
- `cdylib` + `staticlib` crate types.
- FFI smoke (Rust side): `tests/ffi_smoke.rs` — covers lifecycle, error codes,
  null-handle rejection, string-free safety, composite construction.
- Cross-language smoke (C side): `c/examples/smoke.c` — exercises every entry
  point through `cc -lpheno_bridge`.

### Notes

- PR 3 of the 3-PR forgecode improvement sequence (ADR-096).
- Depends on `thegent-memory` v2 (KooshaPari/thegent#1144).
- See `thegent/docs/specs/cdylib-bridge/v1.md` for the canonical SPEC.