//! Tier 3: original C body versus Rust with real C field adapters.
//! C has undefined behavior for NULL; only the Rust no-op is checked there.
use core::ffi::{c_long, c_uint, c_void};
use obs_scene::ffi::item_set_visibility::{AddActiveChild, RemoveActiveChild, set_visibility_with};
use proptest::prelude::*;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

static PROBE: Mutex<()> = Mutex::new(());

unsafe extern "C" {
    fn oracle_scene_item_set_visibility_probe(
        refs: c_long,
        flags: c_uint,
        queued: c_uint,
        visible: bool,
        setter: unsafe extern "C" fn(*mut c_void, bool, AddActiveChild, RemoveActiveChild),
    ) -> c_uint;
}

unsafe extern "C" fn rust_setter(
    item: *mut c_void,
    visible: bool,
    add: AddActiveChild,
    remove: RemoveActiveChild,
) {
    // SAFETY: the oracle supplies a live item and its recording child operations.
    unsafe { set_visibility_with(item, visible, add, remove) }
}

fn compare(refs: c_long, flags: u32, queued: u32, visible: bool) {
    // SAFETY: the oracle constructs and tears down live real-layout items and
    // mutexes. The mutex held by the test serializes its C trace globals.
    let result = unsafe {
        oracle_scene_item_set_visibility_probe(refs, flags, queued, visible, rust_setter)
    };
    assert_eq!(
        result, 0,
        "refs={refs} flags={flags} queued={queued} visible={visible}"
    );
}

#[test]
fn reference_and_queue_edges_match_c() {
    let _guard = PROBE.lock().unwrap_or_else(|poison| poison.into_inner());
    for refs in [c_long::MIN, -2, -1, 0, 1, 2, c_long::MAX] {
        for flags in 0..4 {
            for queued in [0, 1, 4, 8] {
                for visible in [false, true] {
                    compare(refs, flags, queued, visible);
                }
            }
        }
    }
}

#[test]
fn null_item_is_ignored() {
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    unsafe extern "C" fn add(_: *mut c_void, _: *mut c_void) -> bool {
        CALLS.fetch_add(1, Ordering::Relaxed);
        false
    }
    unsafe extern "C" fn remove(_: *mut c_void, _: *mut c_void) {
        CALLS.fetch_add(1, Ordering::Relaxed);
    }
    // SAFETY: the shim explicitly accepts NULL without accessing an adapter.
    unsafe {
        set_visibility_with(core::ptr::null_mut(), true, add, remove);
        set_visibility_with(core::ptr::null_mut(), false, add, remove);
    }
    assert_eq!(CALLS.load(Ordering::Relaxed), 0);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]
    #[test]
    fn random_states_match_c(refs in any::<c_long>(), flags in 0u32..4, queued in 0u32..=8, visible in any::<bool>()) {
        let _guard = PROBE.lock().unwrap_or_else(|poison| poison.into_inner());
        compare(refs, flags, queued, visible);
    }
}
