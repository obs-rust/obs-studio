//! Opaque C handles and mechanical calls to libobs's private registry seam.
use super::pair::obs_hotkey_t;
use crate::register::{self as core, Kind, Registry};
use std::ffi::{c_char, c_uint, c_void};

/// The unchanged public callback ABI.
pub type Callback = Option<unsafe extern "C" fn(*mut c_void, usize, *mut obs_hotkey_t, bool)>;

unsafe extern "C" {
    fn obs_hotkey_register_lock() -> bool;
    fn obs_hotkey_register_unlock();
    fn obs_hotkey_register_private(owner: *mut c_void) -> bool;
    fn obs_hotkey_register_weak(kind: c_uint, owner: *mut c_void) -> *mut c_void;
    fn obs_hotkey_register_context(kind: c_uint, owner: *mut c_void) -> *mut c_void;
    fn obs_hotkey_register_insert(
        kind: c_uint,
        weak: *mut c_void,
        context: *mut c_void,
        name: *const c_char,
        description: *const c_char,
        callback: Callback,
        data: *mut c_void,
    ) -> usize;
}

struct Bridge {
    name: *const c_char,
    description: *const c_char,
    callback: Callback,
    data: *mut c_void,
}
impl Registry for Bridge {
    type Handle = *mut c_void;
    fn source_private(&mut self, owner: Self::Handle) -> bool {
        // SAFETY: the safe core calls this only for a non-null live source.
        unsafe { obs_hotkey_register_private(owner) }
    }
    fn lock(&mut self) -> bool {
        // SAFETY: the seam handles an unavailable global core.
        unsafe { obs_hotkey_register_lock() }
    }
    fn weak(&mut self, kind: Kind, owner: Self::Handle) -> Self::Handle {
        // SAFETY: the core holds the lock and supplied a live non-null owner.
        unsafe { obs_hotkey_register_weak(kind as c_uint, owner) }
    }
    fn context(&mut self, kind: Kind, owner: Self::Handle) -> Self::Handle {
        // SAFETY: the core holds the lock and supplied a live non-null owner.
        unsafe { obs_hotkey_register_context(kind as c_uint, owner) }
    }
    fn insert(
        &mut self,
        kind: Kind,
        weak: Option<Self::Handle>,
        context: Option<Self::Handle>,
    ) -> usize {
        // SAFETY: registration arguments retain the public caller's validity;
        // owner context and weak reference came from the locked C seam.
        unsafe {
            obs_hotkey_register_insert(
                kind as c_uint,
                weak.unwrap_or(std::ptr::null_mut()),
                context.unwrap_or(std::ptr::null_mut()),
                self.name,
                self.description,
                self.callback,
                self.data,
            )
        }
    }
    fn unlock(&mut self) {
        // SAFETY: the safe core calls this exactly once after a successful lock.
        unsafe { obs_hotkey_register_unlock() }
    }
}

fn register(
    kind: Kind,
    owner: *mut c_void,
    name: *const c_char,
    description: *const c_char,
    callback: Callback,
    data: *mut c_void,
) -> usize {
    core::register(
        &mut Bridge {
            name,
            description,
            callback,
            data,
        },
        kind,
        (!owner.is_null()).then_some(owner),
    )
    .unwrap_or(usize::MAX)
}

/// # Safety
/// Strings and callback/data satisfy the unchanged libobs registration contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_register_frontend(
    name: *const c_char,
    description: *const c_char,
    callback: Callback,
    data: *mut c_void,
) -> usize {
    register(
        Kind::Frontend,
        std::ptr::null_mut(),
        name,
        description,
        callback,
        data,
    )
}

/// # Safety
/// `owner` is null or a live source; strings and callback/data obey libobs's contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_register_source(
    owner: *mut c_void,
    name: *const c_char,
    description: *const c_char,
    callback: Callback,
    data: *mut c_void,
) -> usize {
    register(Kind::Source, owner, name, description, callback, data)
}

/// # Safety
/// `owner` is null or a live output; strings and callback/data obey libobs's contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_register_output(
    owner: *mut c_void,
    name: *const c_char,
    description: *const c_char,
    callback: Callback,
    data: *mut c_void,
) -> usize {
    register(Kind::Output, owner, name, description, callback, data)
}

/// # Safety
/// `owner` is null or a live encoder; strings and callback/data obey libobs's contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_register_encoder(
    owner: *mut c_void,
    name: *const c_char,
    description: *const c_char,
    callback: Callback,
    data: *mut c_void,
) -> usize {
    register(Kind::Encoder, owner, name, description, callback, data)
}

/// # Safety
/// `owner` is null or a live service; strings and callback/data obey libobs's contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_register_service(
    owner: *mut c_void,
    name: *const c_char,
    description: *const c_char,
    callback: Callback,
    data: *mut c_void,
) -> usize {
    register(Kind::Service, owner, name, description, callback, data)
}
