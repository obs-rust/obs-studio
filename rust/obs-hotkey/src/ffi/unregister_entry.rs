//! Thin ABI dispatch through the existing recursive hotkey registry seams.
use crate::unregister_entry::{self, Registry, Removal};

pub type Lock = unsafe extern "C" fn() -> bool;
pub type Remove = unsafe extern "C" fn(usize);
pub type Unlock = unsafe extern "C" fn();

unsafe extern "C" {
    fn obs_hotkey_seam_lock() -> bool;
    fn obs_hotkey_seam_unlock();
    fn obs_hotkey_unregister_entry_single_call(id: usize);
    fn obs_hotkey_unregister_pair_call(id: usize);
}

struct Live {
    lock: Lock,
    single: Remove,
    pair: Remove,
    unlock: Unlock,
}
impl Registry for Live {
    fn lock(&mut self) -> bool {
        // SAFETY: the existing seam locks the registry, or returns false without it.
        unsafe { (self.lock)() }
    }
    fn single(&mut self, id: usize) {
        // SAFETY: the core calls this only while the registry lock is held.
        unsafe { (self.single)(id) };
    }
    fn pair(&mut self, id: usize) {
        // SAFETY: the unchanged C pair helper runs under the same registry lock.
        unsafe { (self.pair)(id) };
    }
    fn unlock(&mut self) {
        // SAFETY: the core balances exactly one successful seam acquisition.
        unsafe { (self.unlock)() };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_unregister(id: usize) {
    obs_hotkey_unregister_entry_request(id);
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_pair_unregister(id: usize) {
    obs_hotkey_pair_unregister_entry_request(id);
}

/// Private static entry for fixtures retaining their own registry namespace.
#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_unregister_entry_request(id: usize) {
    dispatch(id, Removal::Single);
}

/// Private static entry for fixtures retaining their real C pair helper.
#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_pair_unregister_entry_request(id: usize) {
    dispatch(id, Removal::Pair);
}

fn dispatch(id: usize, removal: Removal) {
    // SAFETY: all existing seams operate synchronously on the same C registry.
    unsafe {
        with_seams(
            id,
            removal,
            obs_hotkey_seam_lock,
            obs_hotkey_unregister_entry_single_call,
            obs_hotkey_unregister_pair_call,
            obs_hotkey_seam_unlock,
        );
    }
}

/// Exercise the production ABI dispatch with the original-C recorder's seams.
///
/// # Safety
/// Adapters must share a registry and obey the production lock/lifetime contract.
pub unsafe fn with_seams(
    id: usize,
    removal: Removal,
    lock: Lock,
    single: Remove,
    pair: Remove,
    unlock: Unlock,
) {
    unregister_entry::unregister(
        &mut Live {
            lock,
            single,
            pair,
            unlock,
        },
        id,
        removal,
    );
}
