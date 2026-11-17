//! The source remains opaque; every host operation uses its live C adapter.
use crate::push_hotkeys::{self, Host, Kind};
use core::ffi::c_void;

struct Source {
    source: *mut c_void,
    clock: unsafe extern "C" fn() -> u64,
    lock: unsafe extern "C" fn(*mut c_void),
    append: unsafe extern "C" fn(*mut c_void, u64, bool, bool),
    unlock: unsafe extern "C" fn(*mut c_void),
    store: unsafe extern "C" fn(*mut c_void, bool, bool),
}
impl Host for Source {
    fn clock(&mut self) -> u64 {
        // SAFETY: registration supplies the live libobs monotonic clock.
        unsafe { (self.clock)() }
    }
    fn lock(&mut self) {
        // SAFETY: dispatch's caller supplies a live source for this entire event.
        unsafe { (self.lock)(self.source) }
    }
    fn append(&mut self, timestamp: u64, kind: Kind, pressed: bool) {
        // SAFETY: the safe core holds this source's queue mutex around the append.
        unsafe { (self.append)(self.source, timestamp, kind == Kind::Talk, pressed) }
    }
    fn unlock(&mut self) {
        // SAFETY: the core paired this unlock with its lock on the same source.
        unsafe { (self.unlock)(self.source) }
    }
    fn store(&mut self, kind: Kind, pressed: bool) {
        // SAFETY: the source is live and the core has released the queue mutex.
        unsafe { (self.store)(self.source, kind == Kind::Talk, pressed) }
    }
}
/// # Safety
/// `source` must be a live registered hotkey's source, under the C callback contract.
/// Callbacks must be the live C source adapters and clock for this event.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_push_hotkey_dispatch(
    source: *mut c_void,
    talk: bool,
    pressed: bool,
    clock: unsafe extern "C" fn() -> u64,
    lock: unsafe extern "C" fn(*mut c_void),
    append: unsafe extern "C" fn(*mut c_void, u64, bool, bool),
    unlock: unsafe extern "C" fn(*mut c_void),
    store: unsafe extern "C" fn(*mut c_void, bool, bool),
) {
    push_hotkeys::event(
        &mut Source {
            source,
            clock,
            lock,
            append,
            unlock,
            store,
        },
        if talk { Kind::Talk } else { Kind::Mute },
        pressed,
    );
}
