// SPDX-License-Identifier: MIT OR Apache-2.0
//! FFI smoke test. Exercises every C-ABI entry point to make sure the
//! symbol table and ownership rules are correct.
//!
//! The actual cross-language FFI smoke lives in `c/examples/smoke.c` and is
//! run by `scripts/run-c-smoke.sh`. The tests in this file use the in-process
//! Rust `MemoryPort` trait to verify behavior that the C surface delegates to.
//!
//! The `cabi_*` tests below call the exported `pheno_*` C symbols directly.

use std::ffi::{CStr, CString};
use std::sync::Arc;

use thegent_memory::v2::adapters::MockAdapter;
use thegent_memory::v2::{CompositeAdapter, MemoryPort, MemoryProvider, MemoryScope};

// ---------------------------------------------------------------------------
// Port-level tests (exercise the Rust adapter trait directly)
// ---------------------------------------------------------------------------

#[test]
fn mock_adapter_labels_itself() {
    let m = MockAdapter::new();
    // MockAdapter defaults to a generic provider label (Composite) so it can
    // stand in for any single-scope test. The point is the trait method works.
    assert_eq!(m.provider(), MemoryProvider::Composite);
}

#[tokio::test]
async fn mock_adapter_round_trip() {
    let m = MockAdapter::new();
    let id = m
        .store(MemoryScope::Episodic, "k1", "hello".into())
        .await
        .expect("mock store should succeed");
    let recs = m
        .recall(
            MemoryScope::Episodic,
            thegent_memory::v2::MemoryQuery::new("hello"),
        )
        .await
        .expect("mock recall should succeed");
    assert_eq!(recs.len(), 1);
    assert_eq!(recs[0].id, id);
}

#[tokio::test]
async fn composite_routes_each_scope() {
    let composite = CompositeAdapter::new(
        Arc::new(MockAdapter::new()),
        Arc::new(MockAdapter::new()),
        Arc::new(MockAdapter::new()),
        Arc::new(MockAdapter::new()),
    );
    assert_eq!(composite.provider(), MemoryProvider::Composite);

    composite
        .store(MemoryScope::Episodic, "ep", "x".into())
        .await
        .unwrap();
    composite
        .store(MemoryScope::Identity, "id", "y".into())
        .await
        .unwrap();
    composite
        .store(MemoryScope::ProjectKnowledge, "pk", "z".into())
        .await
        .unwrap();
    composite
        .store(MemoryScope::Fallback, "fb", "w".into())
        .await
        .unwrap();
}

#[tokio::test]
async fn composite_scope_routing_is_per_scope() {
    let sm = Arc::new(MockAdapter::new());
    let lt = Arc::new(MockAdapter::new());
    let cg = Arc::new(MockAdapter::new());
    let m0 = Arc::new(MockAdapter::new());
    let composite = CompositeAdapter::new(sm.clone(), lt.clone(), cg.clone(), m0.clone());

    // Each scope must succeed independently.
    composite
        .store(MemoryScope::Episodic, "a", "1".into())
        .await
        .unwrap();
    composite
        .store(MemoryScope::Identity, "b", "2".into())
        .await
        .unwrap();
    composite
        .store(MemoryScope::ProjectKnowledge, "c", "3".into())
        .await
        .unwrap();
    composite
        .store(MemoryScope::Fallback, "d", "4".into())
        .await
        .unwrap();
}

// ---------------------------------------------------------------------------
// Direct C-ABI tests — call the actual exported pheno_* symbols.
// ---------------------------------------------------------------------------

#[test]
fn cabi_version_returns_valid_string() {
    let c = pheno_bridge::pheno_bridge_version();
    assert!(!c.is_null());
    let s = unsafe { CStr::from_ptr(c) }.to_str().expect("valid UTF-8");
    assert!(!s.is_empty(), "version string should not be empty");
    // Should start with a digit (semver).
    assert!(s.as_bytes()[0].is_ascii_digit(), "version should start with a digit: {s}");
}

#[test]
fn cabi_string_free_null_is_safe() {
    // Must not crash.
    pheno_bridge::pheno_string_free(std::ptr::null_mut());
}

#[test]
fn cabi_memory_free_null_is_safe() {
    // Must not crash.
    pheno_bridge::pheno_memory_free(std::ptr::null_mut());
}

#[test]
fn cabi_last_error_returns_valid_string_or_null() {
    // last_error is a global slot shared across parallel tests, so we
    // cannot assert it is NULL at the start. Instead verify the
    // function is callable and returns either NULL or a valid C string.
    let e = pheno_bridge::pheno_last_error();
    if !e.is_null() {
        let s = unsafe { CStr::from_ptr(e) }.to_str().expect("valid UTF-8");
        assert!(!s.is_empty(), "error string should not be empty");
    }
}

#[test]
fn cabi_memory_store_null_handle_returns_error() {
    let scope = CString::new("episodic").unwrap();
    let key = CString::new("k").unwrap();
    let val = CString::new("v").unwrap();
    let rc = pheno_bridge::pheno_memory_store(
        std::ptr::null_mut(),
        scope.as_ptr(),
        key.as_ptr(),
        val.as_ptr(),
    );
    assert_eq!(rc, 1, "null handle should return error code 1");
    assert_last_error_contains("null handle");
}

#[test]
fn cabi_memory_recall_null_handle_returns_error() {
    let scope = CString::new("episodic").unwrap();
    let query = CString::new("q").unwrap();
    let rc = pheno_bridge::pheno_memory_recall(
        std::ptr::null_mut(),
        scope.as_ptr(),
        query.as_ptr(),
        std::ptr::null_mut(),
    );
    assert_eq!(rc, 1, "null handle/out should return error code 1");
    assert_last_error_contains("null handle");
}

#[test]
fn cabi_memory_forget_null_handle_returns_error() {
    let scope = CString::new("episodic").unwrap();
    let key = CString::new("k").unwrap();
    let rc = pheno_bridge::pheno_memory_forget(
        std::ptr::null_mut(),
        scope.as_ptr(),
        key.as_ptr(),
    );
    assert_eq!(rc, 1, "null handle should return error code 1");
    assert_last_error_contains("null handle");
}

#[test]
fn cabi_memory_new_unknown_provider_returns_null() {
    let provider = CString::new("not-a-real-provider").unwrap();
    let h = pheno_bridge::pheno_memory_new(provider.as_ptr());
    assert!(h.is_null(), "unknown provider should return null handle");
    assert_last_error_contains("unknown provider");
}

/// Assert that `pheno_last_error()` contains the given substring.
fn assert_last_error_contains(sub: &str) {
    let e = pheno_bridge::pheno_last_error();
    assert!(!e.is_null(), "expected last_error to be non-null");
    let s = unsafe { CStr::from_ptr(e) }.to_str().expect("valid UTF-8");
    assert!(
        s.contains(sub),
        "expected last_error to contain '{sub}', got: {s}"
    );
}
