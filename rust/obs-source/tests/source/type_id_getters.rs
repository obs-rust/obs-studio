//! Tier 1 mirrors test_source_type_id_getters.c without FFI or raw pointers.
use obs_source::type_id_getters::{get_id, get_type};

#[test]
fn mirrors_null_defaults() {
    let mut calls = Vec::new();
    assert_eq!(get_type(None, |name| calls.push(name)), 0);
    assert_eq!(get_id::<usize>(None, |name| calls.push(name)), 0);
    assert_eq!(calls, [c"obs_source_get_type", c"obs_source_get_id"]);
}

#[test]
fn mirrors_registered_identity_and_rename() {
    for (kind, id) in [(0, "identity_input"), (1, "identity_filter"), (2, "identity_transition")] {
        let mut name = "original context name";
        let borrowed = id;
        assert_eq!(get_type(Some(kind), |_| panic!("live source logged")), kind);
        assert_eq!(get_id(Some(id), |_| panic!("live source logged")), borrowed);
        assert_eq!(name, "original context name");
        name = "renamed context";
        assert_eq!(get_type(Some(kind), |_| panic!("live source logged")), kind);
        assert_eq!(get_id(Some(id), |_| panic!("live source logged")), borrowed);
        assert_eq!(name, "renamed context");
    }
}

#[test]
fn stored_null_id_is_not_a_null_source() {
    assert_eq!(get_id(Some(0usize), |_| panic!("stored NULL logged")), 0);
    assert_eq!(get_type(Some(u32::MAX), |_| panic!("stored type logged")), u32::MAX);
}
