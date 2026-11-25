//! Exact result and full-state parity of the getters and the centered-texture
//! setter (#5599). NULL is excluded because C dereferences it. No intentional
//! differences.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_source::ffi::source_boolean_queries::{self as rust, BooleanQueryFields};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_source_boolean_layout(layout: *mut usize);
    fn oracle_source_boolean_new(
        centered: bool,
        hidden: bool,
        enabled: bool,
        flags: u32,
    ) -> *mut c_void;
    fn oracle_source_boolean_free(source: *mut c_void);
    fn oracle_source_boolean_unchanged(source: *const c_void) -> bool;
    fn oracle_obs_source_get_texcoords_centered(source: *mut c_void) -> bool;
    fn oracle_obs_source_is_hidden(source: *mut c_void) -> bool;
    fn oracle_obs_source_set_texcoords_centered(source: *mut c_void, centered: bool);
    fn oracle_source_boolean_offset(offset: *mut usize);
    fn oracle_source_boolean_new_filled(
        fill: u8,
        centered: bool,
        hidden: bool,
        enabled: bool,
    ) -> *mut c_void;
    fn oracle_source_boolean_stored(source: *mut c_void, expected: bool) -> bool;
    fn oracle_source_boolean_equal(left: *const c_void, right: *const c_void) -> bool;
    fn obs_source_boolean_query_fields(source: *const c_void) -> BooleanQueryFields;
}

#[test]
fn real_header_view_layout() {
    let mut c = [0; 6];
    // SAFETY: the oracle writes six size_t values into the array.
    unsafe { oracle_source_boolean_layout(c.as_mut_ptr()) };
    assert_eq!(
        c,
        [
            size_of::<BooleanQueryFields>(),
            align_of::<BooleanQueryFields>(),
            offset_of!(BooleanQueryFields, texcoords_centered),
            offset_of!(BooleanQueryFields, hidden),
            size_of::<bool>(),
            align_of::<bool>(),
        ]
    );
}

struct Source(*mut c_void);
impl Drop for Source {
    fn drop(&mut self) {
        // SAFETY: the fixture is allocated once and freed exactly once.
        unsafe { oracle_source_boolean_free(self.0) };
    }
}

fn check(centered: bool, hidden: bool, enabled: bool, flags: u32) {
    // SAFETY: the oracle allocates the complete real C source layout.
    let source = Source(unsafe { oracle_source_boolean_new(centered, hidden, enabled, flags) });
    assert!(!source.0.is_null());
    // SAFETY: both implementations borrow the exclusively owned, live fixture.
    unsafe {
        for _ in 0..2 {
            assert_eq!(
                rust::obs_source_get_texcoords_centered(source.0),
                oracle_obs_source_get_texcoords_centered(source.0)
            );
            assert!(oracle_source_boolean_unchanged(source.0));
            assert_eq!(
                rust::obs_source_is_hidden(source.0),
                oracle_obs_source_is_hidden(source.0)
            );
            assert!(oracle_source_boolean_unchanged(source.0));
        }
    }
}

#[test]
fn characterized_edges() {
    for centered in [false, true] {
        for hidden in [false, true] {
            check(centered, hidden, true, 0xa5a5_5a5a);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(64),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn random_fields_match_c(centered in any::<bool>(), hidden in any::<bool>(), enabled in any::<bool>(), flags in any::<u32>()) {
        check(centered, hidden, enabled, flags);
    }
}

/// The adapter the setter stores through hands back the real header's
/// `texcoords_centered` field: its offset from the source is C's `offsetof`.
#[test]
fn setter_field_is_the_real_header_field() {
    let mut c = [0usize; 2];
    // SAFETY: the oracle writes two size_t values into the array.
    unsafe { oracle_source_boolean_offset(c.as_mut_ptr()) };
    // SAFETY: the oracle allocates the complete real C source layout.
    let source = Source(unsafe { oracle_source_boolean_new_filled(0x5a, false, true, true) });
    assert!(!source.0.is_null());
    // SAFETY: the adapter only forms field addresses of the live fixture.
    let fields = unsafe { obs_source_boolean_query_fields(source.0) };
    let offset = fields.texcoords_centered as usize - source.0 as usize;
    assert_eq!(offset, c[0]);
    assert!(offset + size_of::<bool>() <= c[1]);
}

/// Runs one store sequence on twin fixtures, the original C on one and the
/// Rust export on the other. After each store only the centered byte differs
/// from the snapshot, both fixtures are byte-for-byte equal, and both getters
/// answer the stored value.
fn check_sequence(fill: u8, initial: bool, hidden: bool, enabled: bool, sequence: &[bool]) {
    // SAFETY: the oracle allocates the complete real C source layout.
    let (c, r) = unsafe {
        (
            Source(oracle_source_boolean_new_filled(
                fill, initial, hidden, enabled,
            )),
            Source(oracle_source_boolean_new_filled(
                fill, initial, hidden, enabled,
            )),
        )
    };
    assert!(!c.0.is_null() && !r.0.is_null());
    for &requested in sequence {
        // SAFETY: both implementations store into their own live, exclusively
        // owned fixture.
        unsafe {
            oracle_obs_source_set_texcoords_centered(c.0, requested);
            rust::obs_source_set_texcoords_centered(r.0, requested);
            assert!(oracle_source_boolean_stored(c.0, requested));
            assert!(oracle_source_boolean_stored(r.0, requested));
            assert!(oracle_source_boolean_equal(c.0, r.0));
            assert_eq!(oracle_obs_source_get_texcoords_centered(c.0), requested);
            assert_eq!(rust::obs_source_get_texcoords_centered(r.0), requested);
            assert_eq!(
                rust::obs_source_is_hidden(r.0),
                oracle_obs_source_is_hidden(c.0)
            );
            assert!(oracle_source_boolean_stored(r.0, requested));
        }
    }
}

#[test]
fn setter_characterized_sequences() {
    for fill in [0x00, 0xff, 0x5a] {
        for initial in [false, true] {
            for hidden in [false, true] {
                check_sequence(
                    fill,
                    initial,
                    hidden,
                    !hidden,
                    &[true, true, false, false, true, false],
                );
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(64),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn random_store_sequences_match_c(
        fill in any::<u8>(),
        initial in any::<bool>(),
        hidden in any::<bool>(),
        enabled in any::<bool>(),
        sequence in proptest::collection::vec(any::<bool>(), 0..16),
    ) {
        check_sequence(fill, initial, hidden, enabled, &sequence);
    }
}
