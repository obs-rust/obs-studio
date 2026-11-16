//! Tier 1 mirrors of the registry-query assertions in the unchanged C tests.
use obs_output::encoder_registry_queries::{caps, kind};

#[path = "encoder_registry_queries_parity.rs"]
mod parity;

#[test]
fn mirrors_test_encoder_output_getters_characterization_lookup_by_id() {
    assert_eq!(kind(Some(1)), 1);
    assert_eq!(kind(Some(0)), 0);
    assert_eq!(kind(None), 0);
    assert_eq!(caps(Some(1 << 2)), 1 << 2);
    assert_eq!(caps(None), 0);
}

#[test]
fn mirrors_test_register_short_info_registered_caps() {
    // Short registrations store zero caps, even if input bytes beyond size
    // contain texture/scaling flags. These queries read the registered copy.
    assert_eq!(caps(Some(0)), 0);
    assert_eq!(caps(Some(1 << 2)), 1 << 2);
}

#[test]
fn exact_type_and_capability_bits() {
    for value in [0, 1, i32::MAX] {
        assert_eq!(kind(Some(value)), value);
    }
    for value in [0, 1, 2, 0x8000_0000, 0xaaaa_5555, u32::MAX] {
        assert_eq!(caps(Some(value)), value);
    }
}
