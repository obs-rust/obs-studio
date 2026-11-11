//! #4721: exact original-C parity, including embedded NUL and ID-only cases.
//! No intentional differences. Non-NULL sources require a valid C string.
use core::ffi::{c_char, c_long, c_void};
use obs_scene::ffi::kind::{obs_source_is_group, obs_source_is_scene};
use proptest::prelude::*;

type Predicate = unsafe extern "C" fn(*const c_void) -> bool;
unsafe extern "C" {
    fn oracle_scene_kind_probe(
        id: *const c_char,
        bits: u32,
        refs: c_long,
        scene: Option<Predicate>,
        group: Option<Predicate>,
    ) -> u32;
}

fn check(id: &[u8], bits: u32, refs: c_long, rust: bool) {
    // Preserve embedded NULs; the first terminator defines the identifier.
    let mut terminated = id.to_vec();
    terminated.push(0);
    // SAFETY: C constructs complete live records and compares repeated calls
    // and all source, control and scene bytes. No pointers escape this call.
    assert_eq!(
        unsafe {
            oracle_scene_kind_probe(
                terminated.as_ptr().cast(),
                bits,
                refs,
                rust.then_some(obs_source_is_scene),
                rust.then_some(obs_source_is_group),
            )
        },
        0,
        "id={id:?} bits={bits} refs={refs} rust={rust}"
    );
}

#[test]
fn characterized_identifiers_match_original_c() {
    for id in [
        b"scene".as_slice(),
        b"group",
        b"kind-input",
        b"scene-extra",
        b"group-extra",
        b"Scene",
        b"Group",
        b"plain",
        b"",
        b"scene\0suffix",
        b"group\0suffix",
        b"scen",
        b"grou",
        b"\xff",
        b"scene\xff",
        b"\0scene",
    ] {
        for bits in 0..4 {
            for refs in [
                c_long::MIN,
                c_long::from(-1),
                c_long::from(0),
                c_long::from(17),
                c_long::MAX,
            ] {
                check(id, bits, refs, false);
                check(id, bits, refs, true);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128), failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn arbitrary_identifiers_match_original_c(
        id in prop_oneof![
            Just(b"scene".to_vec()), Just(b"group".to_vec()),
            Just(b"scene\0suffix".to_vec()), Just(b"group\0suffix".to_vec()),
            prop::collection::vec(any::<u8>(), 0..65),
        ],
        bits in any::<u32>(), refs in any::<i32>(),
    ) {
        check(&id, bits, c_long::from(refs), true);
    }
}
