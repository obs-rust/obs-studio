//! Tier 1 module accessors: opaque handles represented by integers.
use obs_output::service_module_lookup::{first, Module};

#[test]
fn lists_empty_and_independent() {
    assert_eq!(first(false, 0, 0), 0);
    assert_eq!(first(true, 0, 0), 0);
    assert_eq!(first(false, 7, 9), 7);
    assert_eq!(first(true, 7, 9), 9);
    assert_eq!(first(false, 0, 9), 0);
    assert_eq!(first(true, 7, 0), 0);
}
#[test]
fn links_ids_and_states_are_unchanged() {
    for state in [0, 1, 2, 3, 4, 5] {
        let ids = [11, 11, 22];
        let module = Module { next: 42, services: &ids, load_state: state };
        assert_eq!(module.next(), 42);
        assert_eq!(module.service_count(), 3);
        for (i, id) in ids.iter().enumerate() { assert_eq!(module.service_id(i), *id); }
        assert_eq!(module.load_state(), state);
        assert_eq!(module.service_count(), 3);
    }
    let empty = Module { next: 0, services: &[], load_state: 0 };
    assert_eq!(empty.next(), 0);
    assert_eq!(empty.service_count(), 0);
}
