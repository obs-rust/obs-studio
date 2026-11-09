//! The source remains opaque; every host operation uses its live C adapter.
use crate::push_hotkeys::{self, Host, Kind};
use core::ffi::c_void;

unsafe extern "C" {
    fn obs_source_push_hotkey_clock() -> u64;
    fn obs_source_push_hotkey_lock(source: *mut c_void);
    fn obs_source_push_hotkey_append(
        source: *mut c_void,
        timestamp: u64,
        talk: bool,
        pressed: bool,
    );
    fn obs_source_push_hotkey_unlock(source: *mut c_void);
    fn obs_source_push_hotkey_store(source: *mut c_void, talk: bool, pressed: bool);
}
struct Source(*mut c_void);
impl Host for Source {
    fn clock(&mut self) -> u64 {
        // SAFETY: the private adapter reads the libobs monotonic clock.
        unsafe { obs_source_push_hotkey_clock() }
    }
    fn lock(&mut self) {
        // SAFETY: dispatch's caller supplies a live source for this entire event.
        unsafe { obs_source_push_hotkey_lock(self.0) }
    }
    fn append(&mut self, timestamp: u64, kind: Kind, pressed: bool) {
        // SAFETY: the safe core holds this source's queue mutex around the append.
        unsafe { obs_source_push_hotkey_append(self.0, timestamp, kind == Kind::Talk, pressed) }
    }
    fn unlock(&mut self) {
        // SAFETY: the core paired this unlock with its lock on the same source.
        unsafe { obs_source_push_hotkey_unlock(self.0) }
    }
    fn store(&mut self, kind: Kind, pressed: bool) {
        // SAFETY: the source is live and the core has released the queue mutex.
        unsafe { obs_source_push_hotkey_store(self.0, kind == Kind::Talk, pressed) }
    }
}
/// # Safety
/// `source` must be a live registered hotkey's source, under the C callback contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_push_hotkey_dispatch(
    source: *mut c_void,
    talk: bool,
    pressed: bool,
) {
    push_hotkeys::event(
        &mut Source(source),
        if talk { Kind::Talk } else { Kind::Mute },
        pressed,
    );
}
