// SPDX-License-Identifier: MIT OR Apache-2.0
//! `pheno-cdylib-bridge` — a C-ABI shared library exposing the pheno-*
//! Rust crates (memory, config, port-adapter, flags, errors) to Go and
//! other FFI consumers.
//!
//! The primary consumer is the upstream `antinomyhq/forgecode` agent
//! CLI; the bridge lets forgecode load our pure-Rust crates via
//! `cgo` or `plugin.Open()` without rewriting them in Go.
//!
//! # Surface (initial)
//!
//! | C function | Purpose |
//! |---|---|
//! | `pheno_bridge_version` | Returns a static `*const c_char` with semver |
//! | `pheno_last_error`     | Returns a static `*const c_char` with the most recent error |
//! | `pheno_memory_new`     | Open a memory port (`"sm"`, `"letta"`, `"cognee"`, `"mem0"`, `"composite"`) |
//! | `pheno_memory_store`   | Store a value; returns 0 on success, non-zero on failure |
//! | `pheno_memory_recall`  | Run a recall query; sets `*out` to a heap-allocated JSON string |
//! | `pheno_memory_forget`  | Delete (`scope`, `key`) |
//! | `pheno_memory_free`    | Close the port |
//!
//! # Ownership rules
//!
//! - Strings passed in (`scope`, `key`, `value`, `provider`) must be valid
//!   UTF-8 NUL-terminated `*const c_char`.
//! - Strings passed out (`*out`) are heap-allocated and MUST be freed with
//!   `pheno_string_free`. Do NOT call `free()` on them; they were allocated
//!   by `libc::malloc` via Rust.
//! - Handles (`*mut c_void`) are opaque; free them with `pheno_memory_free`.
//!
//! # Errors
//!
//! Every entry point returns `i32`:
//! - `0` = success
//! - `1` = invalid argument (null pointer, bad UTF-8)
//! - `2` = backend error (network / serialization / unavailable)
//! - `3` = internal error (mutex poison, etc.)
//!
//! Call `pheno_last_error()` for a static error message (do NOT free it).
//!
//! # Threading
//!
//! The bridge is `Send + Sync` over the ports. Multiple threads may call
//! into different handles concurrently. A single handle is also safe to
//! share across threads (the underlying port is async + thread-safe).

// All public functions receive/send raw C pointers; clippy warnings about
// unsafe pointer args are the expected FFI pattern here.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::sync::Arc;

use parking_lot::Mutex;
use thegent_memory::v2::{
    CogneeAdapter, CompositeAdapter, LettaAdapter, MemoryPort, MemoryProvider, MemoryQuery,
    MemoryScope, MemoryValue, Mem0Adapter, SupermemoryAdapter,
};
use tracing::{error, instrument};

/// Opaque handle to a memory port. Internally a `Box<dyn MemoryPort>`,
/// but we hide it as `c_void` per the FFI contract.
struct PortHandle {
    port: Arc<dyn MemoryPort>,
}

/// Last-error slot. `Mutex<Option<String>>` — replaced on every error
/// return. The contents are leaked into a `CString` once per request so
/// the consumer can read the string without lifetime juggling.
static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);

fn record_error(e: impl ToString) {
    let msg = e.to_string();
    error!(msg);
    *LAST_ERROR.lock() = Some(msg);
}

/// Returns the most recent error message as a static `*const c_char`.
/// The string is owned by the bridge; do NOT free it from C. Returns
/// `NULL` if no error has been recorded.
#[instrument(skip_all)]
#[no_mangle]
pub extern "C" fn pheno_last_error() -> *const c_char {
    match LAST_ERROR.lock().as_ref() {
        Some(s) => match CString::new(s.as_str()) {
            Ok(c) => c.into_raw() as *const c_char, // never freed by the bridge
            Err(_) => std::ptr::null(),
        },
        None => std::ptr::null(),
    }
}

/// Free a string returned by the bridge (e.g. `*out` from
/// `pheno_memory_recall`). Calling `free()` directly would work on
/// glibc but is technically UB — use this function to be portable.
#[instrument(skip_all)]
#[no_mangle]
pub extern "C" fn pheno_string_free(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    unsafe {
        let _ = CString::from_raw(s);
    }
}

/// Returns the bridge version as a static `*const c_char`.
#[instrument(skip_all)]
#[no_mangle]
pub extern "C" fn pheno_bridge_version() -> *const c_char {
    // Leaked once; lifetime is `'static` for the process.
    let v = CString::new(env!("CARGO_PKG_VERSION")).expect("static version");
    v.into_raw() as *const c_char
}

/// Parse a `*const c_char` into a Rust `&str`. Returns `Err(1)` on null
/// or invalid UTF-8.
fn cstr<'a>(p: *const c_char) -> Result<&'a str, c_int> {
    if p.is_null() {
        record_error("null pointer");
        return Err(1);
    }
    unsafe {
        CStr::from_ptr(p)
    }
    .to_str()
    .map_err(|e| {
        record_error(format!("invalid UTF-8: {}", e));
        1
    })
}

fn provider_from_str(s: &str) -> Result<MemoryProvider, c_int> {
    match s {
        "sm" | "supermemory" => Ok(MemoryProvider::Supermemory),
        "letta" => Ok(MemoryProvider::Letta),
        "cognee" => Ok(MemoryProvider::Cognee),
        "mem0" => Ok(MemoryProvider::Mem0),
        "composite" => Ok(MemoryProvider::Composite),
        other => {
            record_error(format!("unknown provider: {}", other));
            Err(1)
        }
    }
}

fn scope_from_str(s: &str) -> Result<MemoryScope, c_int> {
    match s {
        "episodic" => Ok(MemoryScope::Episodic),
        "identity" => Ok(MemoryScope::Identity),
        "project_knowledge" | "project" => Ok(MemoryScope::ProjectKnowledge),
        "fallback" => Ok(MemoryScope::Fallback),
        other => {
            record_error(format!("unknown scope: {}", other));
            Err(1)
        }
    }
}

/// Open a memory port.
///
/// `provider` is one of: `"sm"`, `"supermemory"`, `"letta"`, `"cognee"`,
/// `"mem0"`, `"composite"`. For `"composite"`, the bridge constructs a
/// composite that uses the default endpoints for all four primaries;
/// for any other value, returns a single-scope adapter.
///
/// Returns a non-null opaque handle on success, `NULL` on failure.
#[instrument(skip_all)]
#[no_mangle]
pub extern "C" fn pheno_memory_new(provider: *const c_char) -> *mut c_void {
    let provider_str = match cstr(provider) {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    let provider = match provider_from_str(provider_str) {
        Ok(p) => p,
        Err(_) => return std::ptr::null_mut(),
    };

    let port: Arc<dyn MemoryPort> = match provider {
        MemoryProvider::Supermemory => Arc::new(SupermemoryAdapter::default_endpoint()),
        MemoryProvider::Letta => Arc::new(LettaAdapter::default_endpoint()),
        MemoryProvider::Cognee => Arc::new(CogneeAdapter::default_endpoint()),
        MemoryProvider::Mem0 => Arc::new(Mem0Adapter::default_endpoint()),
        MemoryProvider::Composite => Arc::new(CompositeAdapter::new(
            Arc::new(SupermemoryAdapter::default_endpoint()),
            Arc::new(LettaAdapter::default_endpoint()),
            Arc::new(CogneeAdapter::default_endpoint()),
            Arc::new(Mem0Adapter::default_endpoint()),
        )),
    };

    let handle = Box::new(PortHandle { port });
    Box::into_raw(handle) as *mut c_void
}

/// Store `value` under (`scope`, `key`) on the port identified by
/// `handle`. Returns 0 on success, non-zero on failure (see module
/// docs).
#[instrument(skip_all)]
#[no_mangle]
pub extern "C" fn pheno_memory_store(
    handle: *mut c_void,
    scope: *const c_char,
    key: *const c_char,
    value: *const c_char,
) -> c_int {
    if handle.is_null() {
        record_error("null handle");
        return 1;
    }
    let scope_str = match cstr(scope) { Ok(s) => s, Err(c) => return c };
    let key_str = match cstr(key) { Ok(s) => s, Err(c) => return c };
    let value_str = match cstr(value) { Ok(s) => s, Err(c) => return c };
    let scope = match scope_from_str(scope_str) { Ok(s) => s, Err(c) => return c };

    let h = unsafe { &*(handle as *const PortHandle) };

    // Run the async store on a dedicated runtime thread so the FFI
    // caller can stay synchronous.
    let port = h.port.clone();
    let value = MemoryValue::Text(value_str.to_string());
    let key = key_str.to_string();
    let res = tokio_block_on(async move { port.store(scope, &key, value).await });

    match res {
        Ok(_id) => 0,
        Err(e) => {
            record_error(e.to_string());
            2
        }
    }
}

/// Run a recall query and serialize the result as JSON. On success,
/// `*out` is set to a heap-allocated C string the caller MUST free
/// with `pheno_string_free`.
#[instrument(skip_all)]
#[no_mangle]
pub extern "C" fn pheno_memory_recall(
    handle: *mut c_void,
    scope: *const c_char,
    query: *const c_char,
    out: *mut *mut c_char,
) -> c_int {
    if handle.is_null() || out.is_null() {
        record_error("null handle or out");
        return 1;
    }
    let scope_str = match cstr(scope) { Ok(s) => s, Err(c) => return c };
    let query_str = match cstr(query) { Ok(s) => s, Err(c) => return c };
    let scope = match scope_from_str(scope_str) { Ok(s) => s, Err(c) => return c };

    let h = unsafe { &*(handle as *const PortHandle) };

    let port = h.port.clone();
    let q = MemoryQuery::new(query_str.to_string());
    let res = tokio_block_on(async move { port.recall(scope, q).await });

    match res {
        Ok(recs) => match serde_json::to_string(&recs) {
            Ok(s) => match CString::new(s) {
                Ok(c) => {
                    // SAFETY: `out` is a non-null pointer provided by
                    // the caller to receive the result string.
                    unsafe { *out = c.into_raw() };
                    0
                }
                Err(e) => {
                    record_error(format!("CString::new: {}", e));
                    3
                }
            },
            Err(e) => {
                record_error(format!("serde_json::to_string: {}", e));
                3
            }
        },
        Err(e) => {
            record_error(e.to_string());
            2
        }
    }
}

/// Delete (`scope`, `key`).
#[instrument(skip_all)]
#[no_mangle]
pub extern "C" fn pheno_memory_forget(
    handle: *mut c_void,
    scope: *const c_char,
    key: *const c_char,
) -> c_int {
    if handle.is_null() {
        record_error("null handle");
        return 1;
    }
    let scope_str = match cstr(scope) { Ok(s) => s, Err(c) => return c };
    let key_str = match cstr(key) { Ok(s) => s, Err(c) => return c };
    let scope = match scope_from_str(scope_str) { Ok(s) => s, Err(c) => return c };

    let h = unsafe { &*(handle as *const PortHandle) };
    let port = h.port.clone();
    let key = key_str.to_string();
    let res = tokio_block_on(async move { port.forget(scope, &key).await });

    match res {
        Ok(()) => 0,
        Err(e) => {
            record_error(e.to_string());
            2
        }
    }
}

/// Close the port and free the handle. After this call `handle` is
/// invalid.
#[instrument(skip_all)]
#[no_mangle]
pub extern "C" fn pheno_memory_free(handle: *mut c_void) {
    if handle.is_null() {
        return;
    }
    unsafe {
        let _ = Box::from_raw(handle as *mut PortHandle);
    }
}

/// Block on a future using a process-wide Tokio runtime.
fn tokio_block_on<F: std::future::Future>(f: F) -> F::Output {
    use once_cell::sync::OnceCell;
    static RUNTIME: OnceCell<tokio::runtime::Runtime> = OnceCell::new();
    let rt = RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime")
    });
    tokio::task::block_in_place(|| rt.block_on(f))
}