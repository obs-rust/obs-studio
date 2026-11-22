//! `obs_enum_outputs()`, `obs_enum_encoders()` and `obs_enum_services()`
//! over C-owned state: each kind's list mutex, list head and `next` links and
//! the kind's `obs_*_get_ref`/`obs_*_release`, behind the private adapters of
//! `libobs/obs-enum-contexts-fields.c.h`. The snapshot walk is the safe
//! core's ([`crate::enum_contexts::enum_contexts`]); the caller's callback is
//! called here, with its `param` and the entry.
use core::ffi::c_void;
use core::ptr::NonNull;

/// `struct obs_context_data` (the first member of an output, encoder or
/// service), opaque here.
pub type Context = c_void;

/// The caller's callback: `bool (*)(void *param, obs_*_t *entry)`.
pub type EnumProc = Option<unsafe extern "C" fn(*mut c_void, *mut Context) -> bool>;

/// The adapters' kind argument.
const OUTPUTS: u32 = 0;
const ENCODERS: u32 = 1;
const SERVICES: u32 = 2;

unsafe extern "C" {
    fn obs_enum_contexts_lock(kind: u32);
    fn obs_enum_contexts_unlock(kind: u32);
    fn obs_enum_contexts_first(kind: u32) -> *mut Context;
    fn obs_enum_contexts_next(context: *const Context) -> *mut Context;
    fn obs_enum_contexts_get_ref(kind: u32, context: *mut Context) -> *mut Context;
    fn obs_enum_contexts_release(kind: u32, context: *mut Context);
}

struct Kind {
    kind: u32,
    enum_proc: unsafe extern "C" fn(*mut c_void, *mut Context) -> bool,
    param: *mut c_void,
}

impl crate::enum_contexts::Contexts for Kind {
    type Entry = NonNull<Context>;
    fn lock(&mut self) {
        // SAFETY: the initialized core's list mutex for this kind; it is
        // re-entrant, so a callback enumerating again may lock it again.
        unsafe { obs_enum_contexts_lock(self.kind) }
    }
    fn unlock(&mut self) {
        // SAFETY: paired with this walk's own `lock`.
        unsafe { obs_enum_contexts_unlock(self.kind) }
    }
    fn first(&mut self) -> Option<NonNull<Context>> {
        // SAFETY: read under the kind's list mutex, which `lock` holds.
        NonNull::new(unsafe { obs_enum_contexts_first(self.kind) })
    }
    fn next(&mut self, entry: NonNull<Context>) -> Option<NonNull<Context>> {
        // SAFETY: a listed entry, live while the list mutex is held.
        NonNull::new(unsafe { obs_enum_contexts_next(entry.as_ptr()) })
    }
    fn get_ref(&mut self, entry: NonNull<Context>) -> Option<NonNull<Context>> {
        // SAFETY: a listed entry of this kind under its list mutex; a dying
        // one answers NULL.
        NonNull::new(unsafe { obs_enum_contexts_get_ref(self.kind, entry.as_ptr()) })
    }
    fn visit(&mut self, entry: NonNull<Context>) -> bool {
        // SAFETY: the caller's callback and param, as passed; the entry is
        // kept alive by the snapshot's reference. The callback must not
        // unwind across the C boundary.
        unsafe { (self.enum_proc)(self.param, entry.as_ptr()) }
    }
    fn release(&mut self, entry: NonNull<Context>) {
        // SAFETY: a reference this walk took with `get_ref`, released once.
        unsafe { obs_enum_contexts_release(self.kind, entry.as_ptr()) }
    }
}

fn enumerate(kind: u32, enum_proc: EnumProc, param: *mut c_void) {
    // The C asserted a callback; without one there is nothing to call.
    let Some(enum_proc) = enum_proc else {
        return;
    };
    crate::enum_contexts::enum_contexts(&mut Kind {
        kind,
        enum_proc,
        param,
    });
}

/// Calls `enum_proc(param, output)` for a referenced snapshot of the outputs,
/// newest first, until it answers false.
///
/// # Safety
/// As for the C function: the core is initialized, `enum_proc` is a valid
/// callback for `param` that does not unwind.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_enum_outputs(enum_proc: EnumProc, param: *mut c_void) {
    enumerate(OUTPUTS, enum_proc, param);
}

/// As [`obs_enum_outputs`], for the encoders.
///
/// # Safety
/// As for [`obs_enum_outputs`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_enum_encoders(enum_proc: EnumProc, param: *mut c_void) {
    enumerate(ENCODERS, enum_proc, param);
}

/// As [`obs_enum_outputs`], for the services.
///
/// # Safety
/// As for [`obs_enum_outputs`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_enum_services(enum_proc: EnumProc, param: *mut c_void) {
    enumerate(SERVICES, enum_proc, param);
}
