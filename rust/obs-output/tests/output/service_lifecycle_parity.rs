//! Tier 3 for the service lifecycle, references, module lookup and
//! activation (#1920): the Rust shims in obs_output::ffi::service against
//! the original C (rust/obs-c-oracle/oracle/service_getters.c). Both sides
//! call the same test doubles for the steps on struct obs_service, its
//! context and its reference counts, and the same `find_service()` table.
//!
//! Every combination of the 3 optional callbacks these reach (activate,
//! deactivate, initialize) and of a `create()` that fails runs the same
//! scenarios on both sides: creating (unknown type, failed context, public,
//! private), strong and weak refs through the last release, activation,
//! initialization and a destroy deferred while active. The traces (every
//! double and plugin call, in order, with the service's name) and the
//! results must match. The module lookups run over the same module lists.
//!
//! Not compared: the log lines (Tier 1 checks each is made, in order).

use core::cell::RefCell;
use core::ffi::{CStr, c_char, c_int, c_void};
use core::ptr;
use core::sync::atomic::{AtomicBool, Ordering};

use obs_c_oracle::service_getters as c;
use obs_output::ffi::service::{self as rust, ObsServiceInfo};

thread_local! {
    static TRACE: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn trace(event: String) {
    TRACE.with(|t| t.borrow_mut().push(event));
}

fn take_trace() -> Vec<String> {
    TRACE.with(|t| core::mem::take(&mut *t.borrow_mut()))
}

fn name_of(name: *const c_char) -> String {
    if name.is_null() {
        "(null)".into()
    } else {
        // SAFETY: a service name the test passed in.
        unsafe { CStr::from_ptr(name) }
            .to_string_lossy()
            .into_owned()
    }
}

unsafe extern "C" fn hook(event: *const c_char, name: *const c_char) {
    // SAFETY: the double's static event string.
    let event = unsafe { CStr::from_ptr(event) }.to_string_lossy();
    trace(format!("{event} {}", name_of(name)));
}

/// The plugin data `create()` hands out when it succeeds.
static DATA: u8 = 0;
/// Whether `create()` succeeds; set per combination, under the lock.
static CREATE_OK: AtomicBool = AtomicBool::new(true);
/// What `initialize()` returns.
static INIT_RESULT: AtomicBool = AtomicBool::new(false);

fn data_ptr() -> *mut c_void {
    ptr::from_ref(&DATA).cast_mut().cast()
}

unsafe extern "C" fn get_name(_: *mut c_void) -> *const c_char {
    c"parity".as_ptr()
}
unsafe extern "C" fn create(_: *mut c_void, _: *mut c_void) -> *mut c_void {
    trace("create".into());
    if CREATE_OK.load(Ordering::Relaxed) {
        data_ptr()
    } else {
        ptr::null_mut()
    }
}
unsafe extern "C" fn destroy(data: *mut c_void) {
    trace(format!("destroy_cb data={}", data == data_ptr()));
}
unsafe extern "C" fn protocol(_: *mut c_void) -> *const c_char {
    c"RTMP".as_ptr()
}
unsafe extern "C" fn activate(data: *mut c_void, _: *mut c_void) {
    trace(format!("activate_cb data={}", data == data_ptr()));
}
unsafe extern "C" fn deactivate(data: *mut c_void) {
    trace(format!("deactivate_cb data={}", data == data_ptr()));
}
unsafe extern "C" fn initialize(data: *mut c_void, output: *mut c_void) -> bool {
    trace(format!(
        "initialize_cb data={} output={}",
        data == data_ptr(),
        output == output_ptr()
    ));
    INIT_RESULT.load(Ordering::Relaxed)
}

unsafe extern "C" {
    fn oracle_service_state_test_output() -> *mut c_void;
}

fn output_ptr() -> *mut c_void {
    // SAFETY: a real, static C output used only under SERVICE_GLOBALS.
    unsafe { oracle_service_state_test_output() }
}

const ID: &CStr = c"parity_service";

fn info(mask: u32) -> ObsServiceInfo {
    let mut info: ObsServiceInfo =
        // SAFETY: all-zero is a valid table (NULL pointers, None callbacks).
        unsafe { core::mem::zeroed() };
    info.id = ID.as_ptr();
    info.get_name = Some(get_name);
    info.create = Some(create);
    info.destroy = Some(destroy);
    info.get_protocol = Some(protocol);
    if mask & 1 != 0 {
        info.activate = Some(activate);
    }
    if mask & 2 != 0 {
        info.deactivate = Some(deactivate);
    }
    if mask & 4 != 0 {
        info.initialize = Some(initialize);
    }
    info
}

type Create4 =
    unsafe extern "C" fn(*const c_char, *const c_char, *mut c_void, *mut c_void) -> *mut c_void;
type Create3 = unsafe extern "C" fn(*const c_char, *const c_char, *mut c_void) -> *mut c_void;
type Svc = unsafe extern "C" fn(*mut c_void);
type SvcToPtr = unsafe extern "C" fn(*mut c_void) -> *mut c_void;

/// One side's functions.
struct Api {
    create: Create4,
    create_private: Create3,
    destroy: Svc,
    activate: Svc,
    deactivate: unsafe extern "C" fn(*mut c_void, bool),
    initialize: unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool,
    release: Svc,
    weak_addref: Svc,
    weak_release: Svc,
    get_ref: SvcToPtr,
    get_weak: SvcToPtr,
    weak_get: SvcToPtr,
    references: unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool,
    get_module: unsafe extern "C" fn(*const c_char) -> *mut c_void,
    load_state: unsafe extern "C" fn(*const c_char) -> c_int,
}

const RUST: Api = Api {
    create: rust::obs_service_create,
    create_private: rust::obs_service_create_private,
    destroy: rust::obs_service_destroy,
    activate: rust::obs_service_activate,
    deactivate: rust::obs_service_deactivate,
    initialize: rust::obs_service_initialize,
    release: rust::obs_service_release,
    weak_addref: rust::obs_weak_service_addref,
    weak_release: rust::obs_weak_service_release,
    get_ref: rust::obs_service_get_ref,
    get_weak: rust::obs_service_get_weak_service,
    weak_get: rust::obs_weak_service_get_service,
    references: rust::obs_weak_service_references_service,
    get_module: rust::obs_service_get_module,
    load_state: rust::obs_service_load_state,
};

const C: Api = Api {
    create: c::oracle_obs_service_create,
    create_private: c::oracle_obs_service_create_private,
    destroy: c::oracle_obs_service_destroy,
    activate: c::oracle_obs_service_activate,
    deactivate: c::oracle_obs_service_deactivate,
    initialize: c::oracle_obs_service_initialize,
    release: c::oracle_obs_service_release,
    weak_addref: c::oracle_obs_weak_service_addref,
    weak_release: c::oracle_obs_weak_service_release,
    get_ref: c::oracle_obs_service_get_ref,
    get_weak: c::oracle_obs_service_get_weak_service,
    weak_get: c::oracle_obs_weak_service_get_service,
    references: c::oracle_obs_weak_service_references_service,
    get_module: c::oracle_obs_service_get_module,
    load_state: c::oracle_obs_service_load_state,
};

/// The scenarios, on one side; returns the trace with the results in it.
///
/// # Safety
///
/// Under the lock, with the types table and hook set.
unsafe fn scenarios(api: &Api) -> Vec<String> {
    let r = |event: String| trace(format!("=> {event}"));
    // SAFETY: every call below is one side's function on services it
    // created, or NULL, as the C allows.
    unsafe {
        // Unknown type; context that fails.
        r(format!(
            "missing null={}",
            (api.create)(
                c"nope".as_ptr(),
                c"m".as_ptr(),
                ptr::null_mut(),
                ptr::null_mut()
            )
            .is_null()
        ));
        c::oracle_service_set_context_fails(true);
        r(format!(
            "no context null={}",
            (api.create_private)(ID.as_ptr(), c"f".as_ptr(), ptr::null_mut()).is_null()
        ));
        c::oracle_service_set_context_fails(false);

        // Refs through the last release.
        let s = (api.create)(
            ID.as_ptr(),
            c"pub".as_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
        );
        r(format!("pub null={}", s.is_null()));
        let strong = (api.get_ref)(s);
        r(format!("get_ref same={}", strong == s));
        let weak = (api.get_weak)(s);
        r(format!(
            "weak is control={}",
            weak == c::oracle_service_control(s)
        ));
        r(format!("references={}", (api.references)(weak, s)));
        r(format!(
            "references null={}",
            (api.references)(ptr::null_mut(), s)
        ));
        let again = (api.weak_get)(weak);
        r(format!("weak_get same={}", again == s));
        (api.release)(again);
        (api.release)(strong);
        (api.release)(s);
        r(format!("weak_get after={}", (api.weak_get)(weak).is_null()));
        (api.weak_addref)(weak);
        (api.weak_release)(weak);
        (api.weak_release)(weak);
        (api.release)(ptr::null_mut());
        (api.weak_addref)(ptr::null_mut());
        (api.weak_release)(ptr::null_mut());
        r(format!(
            "null refs={}",
            (api.get_ref)(ptr::null_mut()).is_null()
        ));
        r(format!(
            "null weak={}",
            (api.get_weak)(ptr::null_mut()).is_null() && (api.weak_get)(ptr::null_mut()).is_null()
        ));

        // Activation, initialization and a destroy deferred while active.
        let s = (api.create_private)(ID.as_ptr(), c"priv".as_ptr(), ptr::null_mut());
        (api.activate)(ptr::null_mut());
        (api.activate)(s);
        r(format!(
            "no output active={}",
            c::oracle_service_is_active(s)
        ));
        c::oracle_service_set_state(s, false, output_ptr());
        (api.activate)(s);
        (api.activate)(s);
        r(format!("active={}", c::oracle_service_is_active(s)));
        r(format!(
            "init null={}",
            (api.initialize)(s, ptr::null_mut())
        ));
        r(format!(
            "init no service={}",
            (api.initialize)(ptr::null_mut(), output_ptr())
        ));
        r(format!("init={}", (api.initialize)(s, output_ptr())));
        (api.deactivate)(s, false);
        r(format!(
            "output kept={}",
            !c::oracle_service_output(s).is_null()
        ));
        (api.activate)(s);
        (api.deactivate)(s, true);
        r(format!(
            "output removed={}",
            c::oracle_service_output(s).is_null()
        ));
        (api.deactivate)(s, true);
        c::oracle_service_set_state(s, true, output_ptr());
        (api.release)(s);
        r(format!("deferred={}", c::oracle_service_is_destroying(s)));
        (api.deactivate)(s, false);
        (api.destroy)(ptr::null_mut());
        (api.deactivate)(ptr::null_mut(), true);
    }
    take_trace()
}

#[test]
fn lifecycle_matches_c_for_every_callback_combination() {
    let _globals = super::SERVICE_GLOBALS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    // SAFETY: the hook is a plain function; cleared below.
    unsafe { c::oracle_service_set_trace(Some(hook)) };
    for mask in 0..8 {
        for create_ok in [true, false] {
            for init_result in [true, false] {
                let types = [info(mask)];
                CREATE_OK.store(create_ok, Ordering::Relaxed);
                INIT_RESULT.store(init_result, Ordering::Relaxed);
                // SAFETY: under the lock; the table outlives the scenarios.
                unsafe { c::oracle_service_set_types(types.as_ptr().cast(), types.len()) };
                take_trace();
                // SAFETY: under the lock, with the table and hook set.
                let (from_c, from_rust) = unsafe { (scenarios(&C), scenarios(&RUST)) };
                assert!(
                    from_c.iter().any(|e| e.starts_with("free "))
                        && from_c.iter().any(|e| e.starts_with("weak_free ")),
                    "the scenarios reach the frees: {from_c:?}"
                );
                assert_eq!(
                    from_rust, from_c,
                    "mask {mask:#05b}, create ok {create_ok}, initialize {init_result}"
                );
            }
        }
    }
    // SAFETY: under the lock.
    unsafe {
        c::oracle_service_set_types(ptr::null(), 0);
        c::oracle_service_set_trace(None);
    }
}

#[test]
fn module_lookup_matches_c() {
    let _globals = super::SERVICE_GLOBALS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let a: [*const c_char; 2] = [c"a".as_ptr(), c"both".as_ptr()];
    let b: [*const c_char; 1] = [c"b".as_ptr()];
    let both: [*const c_char; 1] = [c"both".as_ptr()];
    // SAFETY: the id arrays outlive the modules; freed below.
    let (m2, m1, d2, d1) = unsafe {
        let m2 = c::oracle_module_new(a.as_ptr(), a.len(), 1, ptr::null_mut());
        let m1 = c::oracle_module_new(ptr::null(), 0, 1, m2);
        let d2 = c::oracle_module_new(both.as_ptr(), both.len(), 3, ptr::null_mut());
        let d1 = c::oracle_module_new(b.as_ptr(), b.len(), 4, d2);
        (m2, m1, d2, d1)
    };
    for (first, disabled) in [
        (m1, d1),
        (ptr::null_mut(), d1),
        (m1, ptr::null_mut()),
        (ptr::null_mut(), ptr::null_mut()),
    ] {
        // SAFETY: under the lock; the lists are live.
        unsafe { c::oracle_service_set_modules(first, disabled) };
        for id in [c"a", c"b", c"both", c"none", c""] {
            // SAFETY: C strings, live module lists.
            unsafe {
                assert_eq!(
                    (RUST.get_module)(id.as_ptr()),
                    (C.get_module)(id.as_ptr()),
                    "get_module {id:?}"
                );
                assert_eq!(
                    (RUST.load_state)(id.as_ptr()),
                    (C.load_state)(id.as_ptr()),
                    "load_state {id:?}"
                );
            }
        }
    }
    // SAFETY: under the lock; nothing reads the modules afterwards.
    unsafe {
        c::oracle_service_set_modules(ptr::null_mut(), ptr::null_mut());
        for m in [m1, m2, d1, d2] {
            c::oracle_module_free(m);
        }
    }
}
