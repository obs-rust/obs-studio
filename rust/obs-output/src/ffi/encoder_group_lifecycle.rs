//! Private-layout and native-operation bridge for encoder-group lifecycle.
use crate::encoder_group_lifecycle::{self as lifecycle, Backend};
use core::ffi::c_void;

#[repr(C)]
pub struct GroupFields {
    pub started: *mut u32,
    pub destroy_on_stop: *mut bool,
}

unsafe extern "C" {
    fn obs_encoder_group_lifecycle_fields(group: *mut c_void) -> GroupFields;
    fn obs_encoder_group_lifecycle_alloc() -> *mut c_void;
    fn obs_encoder_group_lifecycle_free(group: *mut c_void);
    fn obs_encoder_group_lifecycle_init(group: *mut c_void) -> bool;
    fn obs_encoder_group_lifecycle_lock(group: *mut c_void);
    fn obs_encoder_group_lifecycle_unlock(group: *mut c_void);
    fn obs_encoder_group_lifecycle_mutex_destroy(group: *mut c_void);
    fn obs_encoder_group_lifecycle_count(group: *mut c_void) -> usize;
    fn obs_encoder_group_lifecycle_encoder(group: *mut c_void, index: usize) -> *mut c_void;
    fn obs_encoder_group_lifecycle_clear(encoder: *mut c_void);
    fn obs_encoder_group_lifecycle_free_list(group: *mut c_void);
    fn obs_encoder_release(encoder: *mut c_void);
}
struct Native;
impl Backend for Native {
    type Group = *mut c_void;
    fn allocate_zeroed(&mut self) -> Self::Group {
        // SAFETY: libobs allocation has the original abort-on-OOM contract.
        unsafe { obs_encoder_group_lifecycle_alloc() }
    }
    fn init_mutex(&mut self, group: Self::Group) -> bool {
        // SAFETY: the core passes its fresh zeroed group exactly once.
        unsafe { obs_encoder_group_lifecycle_init(group) }
    }
    fn free_group(&mut self, group: Self::Group) {
        // SAFETY: the core has released the list and mutex, or init failed.
        unsafe { obs_encoder_group_lifecycle_free(group) }
    }
    fn lock(&mut self, group: Self::Group) {
        // SAFETY: the caller keeps this initialized native group alive.
        unsafe { obs_encoder_group_lifecycle_lock(group) }
    }
    fn unlock(&mut self, group: Self::Group) {
        // SAFETY: the core owns the lock, including the stop-path handoff.
        unsafe { obs_encoder_group_lifecycle_unlock(group) }
    }
    fn destroy_mutex(&mut self, group: Self::Group) {
        // SAFETY: the core just unlocked the mutex and no owner remains.
        unsafe { obs_encoder_group_lifecycle_mutex_destroy(group) }
    }
    fn started(&mut self, group: Self::Group) -> u32 {
        // SAFETY: the core holds the mutex; the private adapter borrows live fields.
        unsafe { *obs_encoder_group_lifecycle_fields(group).started }
    }
    fn defer(&mut self, group: Self::Group) {
        // SAFETY: the core holds the mutex and the group stays alive.
        unsafe {
            *obs_encoder_group_lifecycle_fields(group).destroy_on_stop = true;
        }
    }
    fn count(&mut self, group: Self::Group) -> usize {
        // SAFETY: the list belongs to the live, locked group.
        unsafe { obs_encoder_group_lifecycle_count(group) }
    }
    fn clear_back_pointer(&mut self, group: Self::Group, index: usize) {
        // SAFETY: the core bounds the index by the locked list's count.
        unsafe {
            obs_encoder_group_lifecycle_clear(obs_encoder_group_lifecycle_encoder(group, index))
        }
    }
    fn release(&mut self, group: Self::Group, index: usize) {
        // SAFETY: the group owns this strong reference and its back-pointer is cleared.
        unsafe { obs_encoder_release(obs_encoder_group_lifecycle_encoder(group, index)) }
    }
    fn free_list(&mut self, group: Self::Group) {
        // SAFETY: all strong references were released under the mutex.
        unsafe { obs_encoder_group_lifecycle_free_list(group) }
    }
}

/// # Safety
/// The returned native allocation must be destroyed with the group API.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_group_create() -> *mut c_void {
    lifecycle::create(&mut Native).unwrap_or(core::ptr::null_mut())
}

/// # Safety
/// `group` must be live with its mutex locked, and the caller must own teardown.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_group_actually_destroy(group: *mut c_void) {
    lifecycle::actually_destroy(&mut Native, group);
}

/// # Safety
/// A non-null group must be live and obey the original C synchronization contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_group_destroy(group: *mut c_void) {
    lifecycle::destroy(&mut Native, (!group.is_null()).then_some(group));
}
