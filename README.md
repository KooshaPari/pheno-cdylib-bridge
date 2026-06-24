# pheno-cdylib-bridge

C-ABI shared library exposing the pheno-* Rust crates (memory, config, port-adapter, flags, errors) to Go and other FFI consumers.

The primary consumer is the upstream [`antinomyhq/forgecode`](https://github.com/antinomyhq/forgecode) agent CLI; the bridge lets forgecode load our pure-Rust crates via `cgo` or `plugin.Open()` without rewriting them in Go.

## Status

v0.1.0 — initial scaffold. PR 3 of the 3-PR forgecode improvement sequence (ADR-096).

## Build

```bash
cargo build --release
# produces:
#   target/release/libpheno_bridge.{so,dylib,dll}  (cdylib)
#   target/release/libpheno_bridge.a              (staticlib)
```

## C ABI surface (v0.1.0)

| C function | Purpose |
|---|---|
| `pheno_bridge_version()` | Static `*const c_char` with the bridge semver |
| `pheno_last_error()` | Static `*const c_char` with the most recent error message |
| `pheno_string_free(s)` | Free a string returned by the bridge |
| `pheno_memory_new(provider)` | Open a memory port (`"sm"`, `"letta"`, `"cognee"`, `"mem0"`, `"composite"`) |
| `pheno_memory_store(h, scope, key, value)` | Store; 0 = success |
| `pheno_memory_recall(h, scope, query, *out)` | Run a recall; `*out` is a JSON string |
| `pheno_memory_forget(h, scope, key)` | Delete; idempotent |
| `pheno_memory_free(h)` | Close the port |

## Ownership rules

- Strings passed in (`scope`, `key`, `value`, `provider`) must be valid UTF-8 NUL-terminated `*const c_char`.
- Strings passed out (`*out`) are heap-allocated; free with `pheno_string_free`, NOT `free()`.
- Handles (`*mut c_void`) are opaque; free them with `pheno_memory_free`.

## Error codes

- `0` = success
- `1` = invalid argument (null pointer, bad UTF-8, unknown enum)
- `2` = backend error (network / serialization / unavailable)
- `3` = internal error (mutex poison, etc.)

Call `pheno_last_error()` for the message string.

## Tests

```bash
cargo test                                    # FFI smoke (Rust side)
bash scripts/build-c-smoke.sh && bash scripts/run-c-smoke.sh   # C side
```

## Refs

- ADR-096: `docs/adr/2026-06-23/ADR-096-forgecode-improvement.md`
- Master plan: `findings/2026-06-23-forgecode-improvement-plan.md`
- Spec: `thegent/docs/specs/cdylib-bridge/v1.md`
- PR 1: `KooshaPari/pheno-forge-plugins` v0.1.0
- PR 2: `KooshaPari/thegent#1144`

## License

Dual-licensed under MIT or Apache-2.0, at your option.