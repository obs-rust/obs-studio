//! Adapt private encoder layout, mutexes and libobs allocations.
use crate::last_error;
use core::ffi::{CStr, c_char, c_int, c_void};
use obs_util::ffi::bmem::bmemdup;
use obs_util::ffi::darray::bfree;

unsafe extern "C" {
    fn obs_encoder_error_ownership_lock(encoder: *mut c_void) -> *mut *mut c_char;
    fn obs_encoder_error_ownership_unlock(encoder: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}
struct Message(*mut c_char);
impl Drop for Message {
    fn drop(&mut self) {
        // SAFETY: exclusively owns a libobs allocation.
        unsafe { bfree(self.0.cast()) };
    }
}
impl Message {
    fn copy(bytes: &[u8]) -> Self {
        // SAFETY: readable bytes including the terminating NUL, copied by bmem.
        Self(unsafe { bmemdup(bytes.as_ptr().cast(), bytes.len()) }.cast())
    }
    fn bytes(&self) -> &[u8] {
        // SAFETY: Message always owns a NUL-terminated allocation.
        unsafe { CStr::from_ptr(self.0) }.to_bytes_with_nul()
    }
    fn into_raw(self) -> *mut c_char {
        let pointer = self.0;
        core::mem::forget(self);
        pointer
    }
}
struct LockedSlot {
    encoder: *mut c_void,
    slot: *mut *mut c_char,
    owned: Option<Message>,
}
impl LockedSlot {
    unsafe fn new(encoder: *mut c_void) -> Self {
        // SAFETY: caller guarantees a live non-NULL encoder; adapter locks before access.
        let slot = unsafe { obs_encoder_error_ownership_lock(encoder) };
        // SAFETY: the mutex protects the live field until this guard drops.
        let pointer = unsafe { *slot };
        Self {
            encoder,
            slot,
            owned: (!pointer.is_null()).then_some(Message(pointer)),
        }
    }
}
impl Drop for LockedSlot {
    fn drop(&mut self) {
        // SAFETY: guard still holds the encoder lock; transfer ownership back before unlock.
        unsafe {
            *self.slot = self
                .owned
                .take()
                .map_or(core::ptr::null_mut(), Message::into_raw);
            obs_encoder_error_ownership_unlock(self.encoder);
        }
    }
}

/// # Safety
/// A non-NULL encoder must be live. A non-NULL message must be a readable
/// NUL-terminated string independent of the previous error allocation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_set_last_error(encoder: *mut c_void, message: *const c_char) {
    if encoder.is_null() {
        // SAFETY: static NUL-terminated format with no conversion arguments.
        unsafe {
            blog(
                400,
                c"obs_encoder_set_last_error: Null 'encoder' parameter".as_ptr(),
            )
        };
        return;
    }
    // SAFETY: validated encoder and caller's independent readable message.
    unsafe {
        let mut slot = LockedSlot::new(encoder);
        let bytes = if message.is_null() {
            None
        } else {
            Some(CStr::from_ptr(message).to_bytes_with_nul())
        };
        last_error::replace(&mut slot.owned, bytes, Message::copy);
    }
}

/// # Safety
/// Encoder must be live and non-NULL, as required by the original private C helper.
/// Caller owns the returned allocation and releases it with bfree.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_dup_last_error(encoder: *mut c_void) -> *mut c_char {
    // SAFETY: caller guarantees the private helper's non-NULL precondition.
    let slot = unsafe { LockedSlot::new(encoder) };
    last_error::duplicate(slot.owned.as_ref(), |message| {
        Message::copy(message.bytes())
    })
    .map_or(core::ptr::null_mut(), Message::into_raw)
}
