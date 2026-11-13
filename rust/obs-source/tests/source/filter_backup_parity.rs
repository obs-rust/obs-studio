//! Tier 3 runs the original C loop and Rust shim over real private source layouts.
//! The save seam supplies metadata and checks locking; Tier 2 uses real plugins.
//! No intentional differences or excluded valid inputs. No ABI structs are copied.
use core::ffi::{CStr, c_void};
use obs_data::ffi::obs_data::{
    obs_data_array_count, obs_data_array_item, obs_data_array_release, obs_data_get_json,
    obs_data_release,
};
use obs_source::ffi::source_filter_backup::obs_source_backup_filters as backup;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Backup = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_filter_backup_new(values: *const u8, count: usize, grow: bool) -> *mut c_void;
    fn oracle_filter_backup_healthy(source: *mut c_void, saves: usize) -> bool;
    fn oracle_filter_backup_free(source: *mut c_void) -> bool;
    fn oracle_obs_source_backup_filters(source: *mut c_void) -> *mut c_void;
    fn oracle_filter_backup_logs(backup: Backup, get: GetLog, set: SetLog) -> bool;
}

struct Fixture(*mut c_void);
impl Fixture {
    fn new(values: &[u8], grow: bool) -> Self {
        // SAFETY: the oracle copies at most 16 bytes into a fully initialized fixture.
        let fixture =
            Self(unsafe { oracle_filter_backup_new(values.as_ptr(), values.len(), grow) });
        assert!(!fixture.0.is_null());
        fixture
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: owns the allocation and its mutex, with no outstanding source users.
        assert!(unsafe { oracle_filter_backup_free(self.0) });
    }
}

struct Array(*mut c_void);
impl Drop for Array {
    fn drop(&mut self) {
        // SAFETY: owns one array reference returned by backup.
        unsafe { obs_data_array_release(self.0) }
    }
}
struct Item(*mut c_void);
impl Drop for Item {
    fn drop(&mut self) {
        // SAFETY: owns the item reference returned by obs_data_array_item.
        unsafe { obs_data_release(self.0) }
    }
}
impl Array {
    fn count(&self) -> usize {
        // SAFETY: array remains live through this borrow.
        unsafe { obs_data_array_count(self.0) }
    }
    fn item(&self, index: usize) -> Item {
        // SAFETY: callers use an in-bounds index in a live array.
        let item = Item(unsafe { obs_data_array_item(self.0, index) });
        assert!(!item.0.is_null());
        item
    }
}
impl Item {
    fn json(&self) -> String {
        // SAFETY: the item owns the JSON string until its next mutation or release;
        // copy it immediately, while the item is still live.
        unsafe {
            CStr::from_ptr(obs_data_get_json(self.0))
                .to_str()
                .unwrap()
                .to_owned()
        }
    }
}

fn check(values: &[u8], grow: bool) {
    let r = Fixture::new(values, grow);
    let c = Fixture::new(values, grow);
    // SAFETY: both fixtures have real source layouts, live mutexes and bounded lists.
    let (rust, original) = unsafe {
        (
            Array(backup(r.0)),
            Array(oracle_obs_source_backup_filters(c.0)),
        )
    };
    assert!(!rust.0.is_null());
    assert!(!original.0.is_null());
    let count = values.len() + usize::from(grow && !values.is_empty() && values.len() < 16);
    assert_eq!(rust.count(), count);
    assert_eq!(original.count(), count);
    // SAFETY: observers only inspect live fixtures and check the mutex is unlocked.
    unsafe {
        assert!(oracle_filter_backup_healthy(r.0, count));
        assert!(oracle_filter_backup_healthy(c.0, count));
    }
    // Saved objects must remain usable after all their source storage is freed.
    drop(r);
    drop(c);
    for i in 0..count {
        assert_eq!(rust.item(i).json(), original.item(i).json());
    }
    if count != 0 {
        let held_r = rust.item(0);
        let held_c = original.item(0);
        drop(rust);
        drop(original);
        assert_eq!(held_r.json(), held_c.json());
    }
}

#[test]
fn empty_singleton_disabled_order_duplicates_and_callback_growth() {
    for values in [&[][..], &[0], &[1], &[3, 2, 1], &[255, 0, 255], &[7; 16]] {
        check(values, false);
        check(values, true);
    }
}

#[test]
fn null_diagnostics_match_c() {
    if !crate::output::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    // SAFETY: isolated child exclusively owns the log handler, which C restores.
    assert!(unsafe {
        oracle_filter_backup_logs(backup, base_get_log_handler, base_set_log_handler)
    });
}

#[test]
fn repeated_backups_release_temporary_references() {
    if !crate::output::isolated(concat!(
        module_path!(),
        "::repeated_backups_release_temporary_references"
    )) {
        return;
    }
    for implementation in [backup as Backup, oracle_obs_source_backup_filters as Backup] {
        let fixture = Fixture::new(&[3, 2, 1], false);
        // SAFETY: fixture remains live, each returned array owns a reference.
        drop(Array(unsafe { implementation(fixture.0) }));
        // SAFETY: this isolated child is the only user of allocation accounting.
        let baseline = unsafe { obs_util::ffi::bmem::bnum_allocs() };
        for _ in 0..4 {
            // SAFETY: the source and its list/mutex remain live for every call.
            let array = Array(unsafe { implementation(fixture.0) });
            assert_eq!(array.count(), 3);
            drop(array);
            // SAFETY: no other test runs in this isolated allocation-counter probe.
            assert_eq!(unsafe { obs_util::ffi::bmem::bnum_allocs() }, baseline);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn arbitrary_filter_lists_match_c(values in prop::collection::vec(any::<u8>(), 0..=16), grow in any::<bool>()) {
        check(&values, grow);
    }
}
