//! `obs_encoder_group_num_started()` over C-owned state: the group's
//! `mutex` and `num_encoders_started`, behind the private adapters of
//! `libobs/obs-encoder-group-num-started-fields.c.h`.
//!
//! Declared in `obs-internal.h` without `EXPORT`, so the shim stays hidden
//! (`rust-hidden.d/obs-output-encoder_group_num_started.txt`). The lock, the
//! one read and the unlock are the safe core's
//! ([`crate::encoder_group_num_started`]); Rust never sees the group's layout
//! or its pthread mutex.
use core::ffi::c_void;

unsafe extern "C" {
    fn obs_encoder_group_num_started_lock(group: *mut c_void);
    fn obs_encoder_group_num_started_read(group: *const c_void) -> u32;
    fn obs_encoder_group_num_started_unlock(group: *mut c_void);
}

/// A live group, never NULL (the C precondition, which this keeps: the
/// pointer goes to the adapters unchecked, as the original dereferences it).
struct Group(*mut c_void);

impl crate::encoder_group_num_started::StartedCount for Group {
    fn lock(&mut self) {
        // SAFETY: the caller's live group, its mutex initialized and not held
        // by this thread; the core locks once and unlocks once.
        unsafe { obs_encoder_group_num_started_lock(self.0) }
    }
    fn read(&mut self) -> u32 {
        // SAFETY: read under the group's mutex, which `lock` holds.
        unsafe { obs_encoder_group_num_started_read(self.0) }
    }
    fn unlock(&mut self) {
        // SAFETY: called once, after `lock`.
        unsafe { obs_encoder_group_num_started_unlock(self.0) }
    }
}

/// The number of the group's encoders that have started, read under the
/// group's mutex.
///
/// # Safety
/// As for the C function: `group` is a live encoder group, not NULL, with
/// its mutex initialized and not held by this thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_group_num_started(group: *mut c_void) -> u32 {
    crate::encoder_group_num_started::num_started(&mut Group(group))
}
