//! Compare the ABI shim against the unchanged C setter over real source fields.
use core::ffi::{c_long, c_void};
use obs_source::ffi::source_async_rotation::obs_source_set_async_rotation;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_obs_source_set_async_rotation(source: *mut c_void, rotation: c_long);
    fn oracle_source_async_rotation_new(initial: c_long) -> *mut c_void;
    fn oracle_source_async_rotation_value(source: *const c_void) -> c_long;
    fn oracle_source_async_rotation_other_fields_unchanged(source: *const c_void) -> bool;
    fn oracle_source_async_rotation_free(source: *mut c_void);
}

struct Source(*mut c_void);
impl Source {
    fn new(initial: c_long) -> Self {
        // SAFETY: the C fixture creates a zeroed, privately owned real source.
        let value = unsafe { oracle_source_async_rotation_new(initial) };
        assert!(!value.is_null());
        Self(value)
    }
}
impl Drop for Source {
    fn drop(&mut self) {
        // SAFETY: this fixture uniquely owns its allocation and holds no resources.
        unsafe { oracle_source_async_rotation_free(self.0) };
    }
}

fn check(rotations: &[c_long], initial: c_long) {
    let rust = Source::new(initial);
    let c = Source::new(initial);
    for &rotation in rotations {
        // SAFETY: both fixtures remain live and writable for the calls and reads.
        unsafe {
            obs_source_set_async_rotation(rust.0, rotation);
            oracle_obs_source_set_async_rotation(c.0, rotation);
            assert_eq!(
                oracle_source_async_rotation_value(rust.0),
                oracle_source_async_rotation_value(c.0),
                "rotation {rotation}"
            );
            assert!(oracle_source_async_rotation_other_fields_unchanged(rust.0));
            assert!(oracle_source_async_rotation_other_fields_unchanged(c.0));
        }
    }
}

#[test]
fn test_async_rotation_normalize_edges_and_changes() {
    let top = c_long::MAX - c_long::MAX % 90;
    let bottom = c_long::MIN - c_long::MIN % 90;
    check(
        &[
            0,
            90,
            180,
            270,
            -90,
            -180,
            -270,
            360,
            450,
            -450,
            720,
            -360,
            top,
            top - 90,
            top - 180,
            bottom,
            bottom + 90,
            bottom + 180,
            1,
            -1,
            45,
            -45,
            135,
            315,
            359,
            -359,
            361,
            c_long::MAX,
            c_long::MIN,
            c_long::MIN + 1,
            -180,
            450,
            -90,
            0,
        ],
        c_long::MIN,
    );
}

#[test]
fn null_source_is_a_noop() {
    for rotation in [c_long::MIN, -1, 0, 1, c_long::MAX] {
        // SAFETY: both setters explicitly accept NULL as a no-op, before access.
        unsafe {
            obs_source_set_async_rotation(core::ptr::null_mut(), rotation);
            oracle_obs_source_set_async_rotation(core::ptr::null_mut(), rotation);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]
    #[test]
    fn random_signed_long_sequences(initial in any::<c_long>(), rotations in prop::collection::vec(any::<c_long>(), 0..32)) {
        check(&rotations, initial);
    }
}
