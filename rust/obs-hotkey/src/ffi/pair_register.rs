//! Thin C boundary for object-bound hotkey-pair registration. No private
//! structs are mirrored: owner fields come from a real-layout C adapter.
use crate::pair_register::{Kind, Registry};
use core::ffi::{c_char, c_int, c_void};

/// `obs_hotkey_active_func` from the unchanged public header.
pub type ActiveCallback =
    Option<unsafe extern "C" fn(*mut c_void, usize, *mut c_void, bool) -> bool>;
/// The owner kind's weak-reference helper passed to the pair engine.
pub type WeakRef = Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>;
/// One owner kind's field adapter.
type Fields = unsafe extern "C" fn(*mut c_void, *mut WeakRef, *mut *mut c_void);
/// The pair engine call adapter (`register_hotkey_pair_internal`).
type Call = unsafe extern "C" fn(
    c_int,
    *mut c_void,
    WeakRef,
    *mut c_void,
    *const c_char,
    *const c_char,
    *const c_char,
    *const c_char,
    ActiveCallback,
    ActiveCallback,
    *mut c_void,
    *mut c_void,
) -> usize;

unsafe extern "C" {
    fn obs_hotkey_pair_register_encoder_fields(
        owner: *mut c_void,
        weak_ref: *mut WeakRef,
        context: *mut *mut c_void,
    );
    fn obs_hotkey_pair_register_output_fields(
        owner: *mut c_void,
        weak_ref: *mut WeakRef,
        context: *mut *mut c_void,
    );
    fn obs_hotkey_pair_register_service_fields(
        owner: *mut c_void,
        weak_ref: *mut WeakRef,
        context: *mut *mut c_void,
    );
    fn obs_hotkey_pair_register_source_fields(
        owner: *mut c_void,
        weak_ref: *mut WeakRef,
        context: *mut *mut c_void,
    );
    fn obs_hotkey_pair_register_call(
        kind: c_int,
        registerer: *mut c_void,
        weak_ref: WeakRef,
        context: *mut c_void,
        name0: *const c_char,
        description0: *const c_char,
        name1: *const c_char,
        description1: *const c_char,
        func0: ActiveCallback,
        func1: ActiveCallback,
        data0: *mut c_void,
        data1: *mut c_void,
    ) -> usize;
}

/// The caller's borrowed arguments, forwarded unchanged.
#[derive(Clone, Copy)]
struct Request {
    names: [*const c_char; 2],
    descriptions: [*const c_char; 2],
    funcs: [ActiveCallback; 2],
    data: [*mut c_void; 2],
}

/// The adapters one registration goes through.
#[derive(Clone, Copy)]
struct Seams {
    encoder: Fields,
    output: Fields,
    service: Fields,
    source: Fields,
    call: Call,
}

const LIVE: Seams = Seams {
    encoder: obs_hotkey_pair_register_encoder_fields,
    output: obs_hotkey_pair_register_output_fields,
    service: obs_hotkey_pair_register_service_fields,
    source: obs_hotkey_pair_register_source_fields,
    call: obs_hotkey_pair_register_call,
};

impl Registry<Request> for Seams {
    type Owner = *mut c_void;
    type Fields = (WeakRef, *mut c_void);

    fn fields(&mut self, kind: Kind, owner: Self::Owner) -> Self::Fields {
        let adapter = match kind {
            Kind::Encoder => self.encoder,
            Kind::Output => self.output,
            Kind::Service => self.service,
            Kind::Source | Kind::Frontend => self.source,
        };
        let mut weak_ref = None;
        let mut context = core::ptr::null_mut();
        // SAFETY: the core calls this only for a non-null owner of the kind
        // promised by its public entry point; the adapter writes both slots.
        unsafe { adapter(owner, &mut weak_ref, &mut context) };
        (weak_ref, context)
    }

    fn register(
        &mut self,
        kind: Kind,
        owner: Self::Owner,
        (weak_ref, context): Self::Fields,
        request: Request,
    ) -> usize {
        let [name0, name1] = request.names;
        let [description0, description1] = request.descriptions;
        let [func0, func1] = request.funcs;
        let [data0, data1] = request.data;
        // SAFETY: the original engine receives the live owner, its own weak
        // helper and context, and the caller's unchanged borrowed arguments.
        unsafe {
            (self.call)(
                kind.code(),
                owner,
                weak_ref,
                context,
                name0,
                description0,
                name1,
                description1,
                func0,
                func1,
                data0,
                data1,
            )
        }
    }
}

/// Run one registration through the given adapters.
///
/// # Safety
/// `owner` is NULL or a live owner of `kind`; the adapters obey the production
/// contract and the request obeys the original public API.
unsafe fn with_seams(mut seams: Seams, kind: Kind, owner: *mut c_void, request: Request) -> usize {
    crate::pair_register::register(
        &mut seams,
        kind,
        (!owner.is_null()).then_some(owner),
        request,
    )
}

#[allow(clippy::too_many_arguments)]
fn register(
    kind: Kind,
    owner: *mut c_void,
    name0: *const c_char,
    description0: *const c_char,
    name1: *const c_char,
    description1: *const c_char,
    func0: ActiveCallback,
    func1: ActiveCallback,
    data0: *mut c_void,
    data1: *mut c_void,
) -> usize {
    let request = Request {
        names: [name0, name1],
        descriptions: [description0, description1],
        funcs: [func0, func1],
        data: [data0, data1],
    };
    // SAFETY: the public entry points forward their callers' contract.
    unsafe { with_seams(LIVE, kind, owner, request) }
}

/// # Safety
/// `encoder` is NULL or a live encoder; other arguments obey the original API.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn obs_hotkey_pair_register_encoder(
    encoder: *mut c_void,
    name0: *const c_char,
    description0: *const c_char,
    name1: *const c_char,
    description1: *const c_char,
    func0: ActiveCallback,
    func1: ActiveCallback,
    data0: *mut c_void,
    data1: *mut c_void,
) -> usize {
    register(
        Kind::Encoder,
        encoder,
        name0,
        description0,
        name1,
        description1,
        func0,
        func1,
        data0,
        data1,
    )
}

/// # Safety
/// `output` is NULL or a live output; other arguments obey the original API.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn obs_hotkey_pair_register_output(
    output: *mut c_void,
    name0: *const c_char,
    description0: *const c_char,
    name1: *const c_char,
    description1: *const c_char,
    func0: ActiveCallback,
    func1: ActiveCallback,
    data0: *mut c_void,
    data1: *mut c_void,
) -> usize {
    register(
        Kind::Output,
        output,
        name0,
        description0,
        name1,
        description1,
        func0,
        func1,
        data0,
        data1,
    )
}

/// # Safety
/// `service` is NULL or a live service; other arguments obey the original API.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn obs_hotkey_pair_register_service(
    service: *mut c_void,
    name0: *const c_char,
    description0: *const c_char,
    name1: *const c_char,
    description1: *const c_char,
    func0: ActiveCallback,
    func1: ActiveCallback,
    data0: *mut c_void,
    data1: *mut c_void,
) -> usize {
    register(
        Kind::Service,
        service,
        name0,
        description0,
        name1,
        description1,
        func0,
        func1,
        data0,
        data1,
    )
}

/// # Safety
/// `source` is NULL or a live source; other arguments obey the original API.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn obs_hotkey_pair_register_source(
    source: *mut c_void,
    name0: *const c_char,
    description0: *const c_char,
    name1: *const c_char,
    description1: *const c_char,
    func0: ActiveCallback,
    func1: ActiveCallback,
    data0: *mut c_void,
    data1: *mut c_void,
) -> usize {
    register(
        Kind::Source,
        source,
        name0,
        description0,
        name1,
        description1,
        func0,
        func1,
        data0,
        data1,
    )
}
