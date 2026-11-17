//! Borrow private C fields only while the existing registry mutex is held.
use crate::background_control::{BackgroundControl, Request, State, request};

unsafe extern "C" {
    fn obs_hotkey_background_control_lock() -> bool;
    fn obs_hotkey_background_control_retry() -> *mut bool;
    fn obs_hotkey_background_control_configure() -> *mut bool;
    fn obs_hotkey_background_control_probe() -> *mut bool;
    fn obs_hotkey_background_control_flags() -> *mut u32;
    fn obs_hotkey_background_control_wake();
    fn obs_hotkey_background_control_unlock();
}

struct Client;
impl BackgroundControl for Client {
    fn lock(&mut self) -> bool {
        // SAFETY: the real seam checks core presence before locking.
        unsafe { obs_hotkey_background_control_lock() }
    }
    fn read(&self) -> State {
        // SAFETY: request reads only after acquiring the hotkey mutex;
        // the adapter returns live, correctly typed private field addresses.
        unsafe {
            State {
                retry: *obs_hotkey_background_control_retry(),
                configure: *obs_hotkey_background_control_configure(),
                probe: *obs_hotkey_background_control_probe(),
                flags: *obs_hotkey_background_control_flags(),
            }
        }
    }
    fn write(&mut self, state: State) {
        // SAFETY: request writes under the same mutex as read, before waking
        // the client; no Rust references survive the C callback.
        unsafe {
            *obs_hotkey_background_control_retry() = state.retry;
            *obs_hotkey_background_control_configure() = state.configure;
            *obs_hotkey_background_control_probe() = state.probe;
            *obs_hotkey_background_control_flags() = state.flags;
        }
    }
    fn wake(&mut self) {
        // SAFETY: request still owns the mutex, keeping the client live.
        unsafe { obs_hotkey_background_control_wake() };
    }
    fn unlock(&mut self) {
        // SAFETY: request releases each successful lock exactly once.
        unsafe { obs_hotkey_background_control_unlock() };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkeys_retry_background() {
    obs_hotkey_background_control_retry_request();
}

/// Private entry for isolated C fixtures, whose registry has its own namespace.
#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_background_control_retry_request() {
    request(&mut Client, Request::Retry);
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkeys_configure_background() {
    obs_hotkey_background_control_configure_request();
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_background_control_configure_request() {
    request(&mut Client, Request::Configure);
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkeys_set_background_probe(enable: bool) {
    obs_hotkey_background_control_probe_request(enable);
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_background_control_probe_request(enable: bool) {
    request(&mut Client, Request::Probe(enable));
}
