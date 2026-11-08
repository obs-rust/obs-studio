//! Exact byte parity with extracted C; no intentional differences.
//! No structs cross this ABI, so there is no new representation to lay out.
use core::ffi::c_char;
use obs_c_oracle as _;
use obs_scene::ffi::source_type_queries::{obs_source_type_is_group, obs_source_type_is_scene};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_obs_source_type_is_group(id: *const c_char) -> bool;
    fn oracle_obs_source_type_is_scene(id: *const c_char) -> bool;
}

fn check(input: Option<&[u8]>) {
    let mut bytes = input.map(<[u8]>::to_vec).unwrap_or_default();
    bytes.push(0);
    let before = bytes.clone();
    let id = if input.is_some() {
        bytes.as_ptr().cast()
    } else {
        core::ptr::null()
    };
    // SAFETY: NULL is supported, otherwise bytes is live and NUL-terminated.
    unsafe {
        assert_eq!(
            obs_source_type_is_group(id),
            oracle_obs_source_type_is_group(id)
        );
        assert_eq!(
            obs_source_type_is_scene(id),
            oracle_obs_source_type_is_scene(id)
        );
    }
    assert_eq!(bytes, before);
}

#[test]
fn characterized_edges() {
    check(None);
    for bytes in [
        b"".as_slice(),
        b"group",
        b"scene",
        b"Group",
        b"Scene",
        b"grou",
        b"scen",
        b"groups",
        b"scenes",
        b" scene",
        b"group ",
        b"\xff",
        b"group\0ignored",
        b"scene\0ignored",
    ] {
        check(Some(bytes));
    }
    for byte in 0..=u8::MAX {
        check(Some(&[byte]));
        let mut group = b"group".to_vec();
        group.push(byte);
        check(Some(&group));
        let mut scene = b"scene".to_vec();
        scene.push(byte);
        check(Some(&scene));
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn arbitrary_bytes(input in prop_oneof![
        Just(Some(b"group".to_vec())),
        Just(Some(b"scene".to_vec())),
        Just(None),
        proptest::option::of(proptest::collection::vec(any::<u8>(), 0..128)),
    ]) {
        check(input.as_deref());
    }
}
