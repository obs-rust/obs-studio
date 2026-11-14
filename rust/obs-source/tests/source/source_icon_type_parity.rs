//! Tier 3: the public shim versus the original C over every enum and missing IDs.
//! Both sides look IDs up with the ported get_source_info over the icon
//! registry, the shared fixture's default (source_info_lookup.c).
use core::ffi::{c_char, c_uint};
use core::mem::{align_of, size_of};
use obs_source::ffi::source_icon_type::obs_source_get_icon_type;
use proptest::prelude::*;
use std::ffi::CString;

unsafe extern "C" {
    fn oracle_obs_source_get_icon_type(id: *const c_char) -> c_uint;
    fn oracle_source_icon_type_layout(size: *mut usize, alignment: *mut usize) -> bool;
}

fn check(id: &str, expected: u32) {
    let id = CString::new(id).unwrap();
    // SAFETY: both functions borrow a live terminated string and immutable
    // registry entries; the fixture never changes the registry.
    unsafe {
        assert_eq!(
            obs_source_get_icon_type(id.as_ptr()),
            oracle_obs_source_get_icon_type(id.as_ptr())
        );
        assert_eq!(obs_source_get_icon_type(id.as_ptr()), expected);
    }
}

#[test]
fn every_icon_missing_ids_and_real_enum_layout() {
    let (mut size, mut alignment) = (0, 0);
    // SAFETY: both outputs are writable; the helper verifies all field addresses.
    assert!(unsafe { oracle_source_icon_type_layout(&mut size, &mut alignment) });
    assert_eq!(size, size_of::<c_uint>());
    assert_eq!(alignment, align_of::<c_uint>());
    for icon in 0..=14 {
        check(&format!("icon_{icon}"), icon);
    }
    for id in [
        "",
        "missing",
        "ICON_9",
        "icon_15",
        "icon_09",
        "icon_9suffix",
    ] {
        check(id, 0);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]
    #[test]
    fn registered_and_unknown_ids(icon in 0u32..64, suffix in "[a-z]{0,16}") {
        let expected = if icon < 15 && suffix.is_empty() { icon } else { 0 };
        check(&format!("icon_{icon}{suffix}"), expected);
    }
}
