//! Thin C boundary for single-hotkey registration. No private structs are mirrored.
use crate::register::{Kind, Registry};
use core::ffi::{c_char, c_int, c_void};

/// `obs_hotkey_func` from the unchanged public header.
pub type Callback = Option<unsafe extern "C" fn(*mut c_void, usize, *mut c_void, bool)>;

unsafe extern "C" {
    fn obs_hotkey_register_lock() -> bool;
    fn obs_hotkey_register_unlock();
    fn obs_hotkey_register_source_private(owner: *mut c_void) -> bool;
    fn obs_hotkey_register_encoder_fields(
        owner: *mut c_void,
        weak: *mut *mut c_void,
        context: *mut *mut c_void,
    );
    fn obs_hotkey_register_output_fields(
        owner: *mut c_void,
        weak: *mut *mut c_void,
        context: *mut *mut c_void,
    );
    fn obs_hotkey_register_service_fields(
        owner: *mut c_void,
        weak: *mut *mut c_void,
        context: *mut *mut c_void,
    );
    fn obs_hotkey_register_source_fields(
        owner: *mut c_void,
        weak: *mut *mut c_void,
        context: *mut *mut c_void,
    );
    fn obs_hotkey_register_call(
        kind: c_int,
        weak: *mut c_void,
        context: *mut c_void,
        name: *const c_char,
        description: *const c_char,
        func: Callback,
        data: *mut c_void,
    ) -> usize;
}

struct Request {
    name: *const c_char,
    description: *const c_char,
    func: Callback,
    data: *mut c_void,
}
struct Live;
impl Registry<Request> for Live {
    type Owner = *mut c_void;
    type Weak = *mut c_void;
    type Context = *mut c_void;
    fn source_private(&mut self, owner: Self::Owner) -> bool {
        // SAFETY: the safe core checks that the caller's source is non-null first.
        unsafe { obs_hotkey_register_source_private(owner) }
    }
    fn lock(&mut self) -> bool {
        // SAFETY: the real adapter checks core presence before locking.
        unsafe { obs_hotkey_register_lock() }
    }
    fn acquire(&mut self, kind: Kind, owner: Self::Owner) -> (Self::Weak, Self::Context) {
        let mut weak = core::ptr::null_mut();
        let mut context = core::ptr::null_mut();
        // SAFETY: the core calls this only for a non-null owner of the kind
        // promised by its public entry point and with the hotkey mutex held.
        unsafe {
            match kind {
                Kind::Frontend => {}
                Kind::Source => obs_hotkey_register_source_fields(owner, &mut weak, &mut context),
                Kind::Output => obs_hotkey_register_output_fields(owner, &mut weak, &mut context),
                Kind::Encoder => obs_hotkey_register_encoder_fields(owner, &mut weak, &mut context),
                Kind::Service => obs_hotkey_register_service_fields(owner, &mut weak, &mut context),
            }
        }
        (weak, context)
    }
    fn register(
        &mut self,
        kind: Kind,
        owner: Option<(Self::Weak, Self::Context)>,
        request: Request,
    ) -> usize {
        let (weak, context) = owner.unwrap_or((core::ptr::null_mut(), core::ptr::null_mut()));
        // SAFETY: the core holds the lock, and the original engine receives the
        // live owner/context and the caller's unchanged borrowed arguments.
        unsafe {
            obs_hotkey_register_call(
                kind.code(),
                weak,
                context,
                request.name,
                request.description,
                request.func,
                request.data,
            )
        }
    }
    fn unlock(&mut self) {
        // SAFETY: the core releases precisely a successfully acquired lock.
        unsafe { obs_hotkey_register_unlock() }
    }
}

fn register(kind: Kind, owner: *mut c_void, request: Request) -> usize {
    crate::register::register(
        &mut Live,
        kind,
        (!owner.is_null()).then_some(owner),
        request,
    )
}

/// # Safety
/// Strings, callback and data obey the original frontend registration contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_register_frontend(
    name: *const c_char,
    description: *const c_char,
    func: Callback,
    data: *mut c_void,
) -> usize {
    register(
        Kind::Frontend,
        core::ptr::null_mut(),
        Request {
            name,
            description,
            func,
            data,
        },
    )
}

/// # Safety
/// `owner` is NULL or a live source; other arguments obey the original API.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_register_source(
    owner: *mut c_void,
    name: *const c_char,
    description: *const c_char,
    func: Callback,
    data: *mut c_void,
) -> usize {
    register(
        Kind::Source,
        owner,
        Request {
            name,
            description,
            func,
            data,
        },
    )
}

/// # Safety
/// `owner` is NULL or a live output; other arguments obey the original API.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_register_output(
    owner: *mut c_void,
    name: *const c_char,
    description: *const c_char,
    func: Callback,
    data: *mut c_void,
) -> usize {
    register(
        Kind::Output,
        owner,
        Request {
            name,
            description,
            func,
            data,
        },
    )
}

/// # Safety
/// `owner` is NULL or a live encoder; other arguments obey the original API.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_register_encoder(
    owner: *mut c_void,
    name: *const c_char,
    description: *const c_char,
    func: Callback,
    data: *mut c_void,
) -> usize {
    register(
        Kind::Encoder,
        owner,
        Request {
            name,
            description,
            func,
            data,
        },
    )
}

/// # Safety
/// `owner` is NULL or a live service; other arguments obey the original API.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_register_service(
    owner: *mut c_void,
    name: *const c_char,
    description: *const c_char,
    func: Callback,
    data: *mut c_void,
) -> usize {
    register(
        Kind::Service,
        owner,
        Request {
            name,
            description,
            func,
            data,
        },
    )
}
