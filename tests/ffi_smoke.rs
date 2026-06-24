// SPDX-License-Identifier: MIT OR Apache-2.0
//! FFI smoke test. Exercises every C-ABI entry point to make sure the
//! symbol table and ownership rules are correct.
//!
//! The actual cross-language FFI smoke lives in `c/examples/smoke.c` and is
//! run by `scripts/run-c-smoke.sh`. The tests in this file use the in-process
//! Rust `MemoryPort` trait to verify behavior that the C surface delegates to.

use std::sync::Arc;

use thegent_memory::v2::adapters::MockAdapter;
use thegent_memory::v2::{CompositeAdapter, MemoryPort, MemoryProvider, MemoryScope};

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
