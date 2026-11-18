//! Tier 1: assertions from test/cmocka/test_source_weak_predicates.c.
use obs_source::weak_predicates::{expired, references};

#[test]
fn test_source_weak_predicates_null_weak() {
    assert!(expired(None));
    assert!(!references::<usize>(None, None));
}

#[test]
fn test_source_weak_predicates_live_identity() {
    let source = Some(1usize);
    let other = Some(2usize);
    // Safe identities stand for the C fixture's two allocated source handles.
    assert!(source.is_some());
    assert!(other.is_some());
    let weak = source;
    let other_weak = other;
    assert!(weak.is_some());
    assert!(other_weak.is_some());
    for _ in 0..3 {
        assert!(!expired(Some(0)));
        assert!(!expired(Some(0)));
        assert!(references(weak, source));
        assert!(!references(weak, other));
        assert!(references(other_weak, other));
        assert!(!references(other_weak, source));
        assert!(!references(weak, None));
        assert!(!references(None, source));
    }
    // Dropping the last owners changes only the biased count.
    assert!(expired(Some(-1)));
    assert!(expired(Some(-1)));
}

#[test]
fn test_source_weak_predicates_last_strong_release() {
    let source = Some(1usize);
    let other = Some(2usize);
    assert!(source.is_some());
    assert!(other.is_some());
    let retained_source = source;
    let weak = source;
    assert!(retained_source.is_some());
    assert!(weak.is_some());
    // One strong reference remains after the scene releases its owner.
    assert!(!expired(Some(0)));
    assert!(references(weak, retained_source));
    // The C task keeps allocation alive while probing after last release.
    let probe_expired = expired(Some(-1));
    let probe_same = references(weak, retained_source);
    let probe_different = references(weak, other);
    let probe_null = references(weak, None);
    assert!(probe_expired);
    assert!(probe_same);
    assert!(!probe_different);
    assert!(!probe_null);
    assert!(expired(Some(-1)));
    // Once expired, an owner cannot be upgraded; no freed identity is used.
    let upgraded = (!expired(Some(-1))).then_some(1usize);
    assert!(upgraded.is_none());
    assert!(!references(weak, other));
    assert!(!references(weak, None));
}

#[test]
fn biased_count_boundaries_and_absent_stored_identity() {
    for owners in [i64::MIN, -2, -1] {
        assert!(expired(Some(owners)));
    }
    for owners in [0, 1, i64::MAX] {
        assert!(!expired(Some(owners)));
    }
    assert!(!references(None, Some(1usize)));
}
