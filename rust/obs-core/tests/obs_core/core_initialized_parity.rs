//! Exact parity: the pointer is compared with NULL, never dereferenced.
use obs_core::ffi::core_initialized::obs_initialized;
use proptest::prelude::*;
use std::ffi::c_void;
use std::sync::Mutex;

static LOCK: Mutex<()> = Mutex::new(());

unsafe extern "C" {
    fn oracle_core_initialized_set(core: *mut c_void);
    fn oracle_core_initialized_layout(layout: *mut usize);
    fn oracle_obs_initialized() -> bool;
}

#[test]
fn real_header_scalar_layout() {
    let mut layout = [0; 4];
    // SAFETY: the oracle writes exactly four size_t entries.
    unsafe { oracle_core_initialized_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<*const c_void>(),
            align_of::<*const c_void>(),
            size_of::<bool>(),
            align_of::<bool>()
        ]
    );
}

fn compare(address: usize) {
    let core = std::ptr::without_provenance_mut::<c_void>(address);
    // SAFETY: the test owns the oracle global under LOCK. Neither implementation
    // dereferences the pointer, and neither changes it. The slot itself is live.
    unsafe {
        oracle_core_initialized_set(core);
        assert_eq!(obs_initialized(), oracle_obs_initialized());
        assert_eq!(obs_initialized(), address != 0);
        assert_eq!(oracle_obs_initialized(), address != 0);
    }
}

#[test]
fn characterized_edges() {
    let _guard = LOCK.lock().unwrap();
    for address in [0, 0, 1, 1, 0, 0, 2, usize::MAX / 2, usize::MAX, 0] {
        compare(address);
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn initialization_matches_c(address in any::<usize>()) {
        let _guard = LOCK.lock().unwrap();
        compare(address);
        compare(0);
    }
}
