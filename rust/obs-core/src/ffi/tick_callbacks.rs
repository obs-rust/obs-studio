//! C-owned tick array and native recursive mutex, borrowed for each operation.
use crate::tick_callbacks::{self, Registry};
use core::ffi::c_void;
use obs_util::ffi::darray::{darray, darray_erase, darray_insert};

pub type Tick = Option<unsafe extern "C" fn(*mut c_void, f32)>;

/// Exact private C row layout; measured against obs-internal.h by the oracle.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Entry {
    pub tick: Tick,
    pub param: *mut c_void,
}
impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.tick.map(|f| f as usize) == other.tick.map(|f| f as usize) && self.param == other.param
    }
}
impl Eq for Entry {}

unsafe extern "C" {
    fn obs_core_initialized_field() -> *const *const c_void;
    fn obs_core_tick_callbacks_array(core: *mut c_void) -> *mut darray;
    fn obs_core_tick_callbacks_lock(core: *mut c_void);
    fn obs_core_tick_callbacks_unlock(core: *mut c_void);
}
struct Access {
    core: *mut c_void,
    array: *mut darray,
}
impl Registry for Access {
    type Entry = Entry;
    fn lock(&mut self) {
        // SAFETY: public API requires a live initialized core. The existing
        // recursive draw mutex protects both this array and C dispatch.
        unsafe {
            obs_core_tick_callbacks_lock(self.core);
            self.array = obs_core_tick_callbacks_array(self.core);
        }
    }
    fn unlock(&mut self) {
        // SAFETY: this thread acquired the same core's mutex in lock().
        unsafe { obs_core_tick_callbacks_unlock(self.core) };
    }
    fn len(&self) -> usize {
        // SAFETY: the policy reads storage only while the draw mutex is held.
        unsafe { (*self.array).num }
    }
    fn get(&self, index: usize) -> Entry {
        // SAFETY: policy bounds the index by len under the draw mutex. Copying
        // the row retains no reference across mutation or recursive dispatch.
        unsafe { (*self.array).array.cast::<Entry>().add(index).read() }
    }
    fn insert(&mut self, index: usize, entry: Entry) {
        // SAFETY: initialized C darray, valid insertion index and readable row.
        // bmalloc-backed storage stays owned by C, including shutdown's da_free.
        unsafe {
            darray_insert(
                size_of::<Entry>(),
                self.array,
                index,
                (&raw const entry).cast(),
            )
        };
    }
    fn erase(&mut self, index: usize) {
        // SAFETY: policy supplies an in-bounds index under the draw mutex.
        unsafe { darray_erase(size_of::<Entry>(), self.array, index) };
    }
}
unsafe fn access() -> Access {
    // SAFETY: caller guarantees the global points to a live core, as the
    // original C API requires. No field accessor is called on NULL.
    let core = unsafe { *obs_core_initialized_field() }.cast_mut();
    assert!(
        !core.is_null(),
        "tick callbacks require an initialized core"
    );
    Access {
        core,
        array: core::ptr::null_mut(),
    }
}

/// Register a callback, retaining duplicates and NULL parameter values.
///
/// # Safety
/// OBS must be initialized throughout the call. Callback/parameter lifetimes
/// must cover dispatch until removal, following the existing C API contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_add_tick_callback(tick: Tick, param: *mut c_void) {
    // SAFETY: the caller guarantees core lifetime and initialization.
    tick_callbacks::add(&mut unsafe { access() }, Entry { tick, param });
}

/// Remove the newest matching registration, if present.
///
/// # Safety
/// OBS must be initialized throughout the call; shutdown must not race it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_remove_tick_callback(tick: Tick, param: *mut c_void) {
    // SAFETY: the caller guarantees core lifetime and initialization.
    tick_callbacks::remove(&mut unsafe { access() }, Entry { tick, param });
}
