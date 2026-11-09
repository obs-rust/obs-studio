//! Opaque source and typed C getter/setter callbacks; no source layout crosses.
use crate::mute_monitor_hotkeys::{self, Control};
use core::ffi::c_void;

/// C source getter, supplied by the static registration adapter.
pub type Getter = unsafe extern "C" fn(*const c_void) -> bool;
/// C source setter, supplied by the static registration adapter.
pub type Setter = unsafe extern "C" fn(*mut c_void, bool);

struct Source {
    source: *mut c_void,
    getter: Getter,
    setter: Setter,
}
impl Control for Source {
    fn enabled(&mut self) -> bool {
        // SAFETY: dispatch's caller supplies the matching getter and live source.
        unsafe { (self.getter)(self.source) }
    }
    fn set_enabled(&mut self, enabled: bool) {
        // SAFETY: the matching C setter receives the same source for this event.
        unsafe { (self.setter)(self.source, enabled) }
    }
}

/// # Safety
/// The source and getter/setter callbacks must remain valid throughout the call;
/// the callbacks must address the same control as the original registration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_mute_monitor_hotkey_dispatch(
    source: *mut c_void,
    _monitor: bool,
    target: bool,
    pressed: bool,
    getter: Getter,
    setter: Setter,
) -> bool {
    mute_monitor_hotkeys::event(
        &mut Source {
            source,
            getter,
            setter,
        },
        target,
        pressed,
    )
}
