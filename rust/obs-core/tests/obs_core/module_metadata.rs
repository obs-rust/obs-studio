use obs_core::module_metadata::{field, name};

// Mirrors null_and_empty in test_module_metadata.c for all six field getters.
#[test]
fn null_and_empty() {
    for _ in 0..6 {
        assert_eq!(field(None, 0usize), 0);
        assert_eq!(field(Some(0usize), 0), 0);
    }
    assert_eq!(name(None, None::<fn() -> usize>, 0usize), 0);
}

// Mirrors borrowed_fields: values are preserved, including an empty value.
#[test]
fn borrowed_fields() {
    for value in [0usize, 1, 17, usize::MAX] {
        assert_eq!(field(Some(value), 0), value);
    }
}

// Mirrors callbacks_and_precedence, including calls returning null/default.
#[test]
fn callbacks_and_precedence() {
    let mut calls = 0;
    for _ in 0..3 {
        assert_eq!(name(None, Some(|| { calls += 1; 17usize }), 0), 17);
    }
    assert_eq!(calls, 3);
    assert_eq!(name(Some(1usize), Some(|| { calls += 1; 17 }), 0), 1);
    assert_eq!(calls, 3);
    assert_eq!(name(None, Some(|| { calls += 1; 17usize }), 0), 17);
    assert_eq!(calls, 4);
    for _ in 0..3 {
        assert_eq!(name(None, Some(|| { calls += 1; 0usize }), 0), 0);
    }
    assert_eq!(calls, 7);
    assert_eq!(name(None, None::<fn() -> usize>, 0usize), 0);
    assert_eq!(calls, 7);
}
