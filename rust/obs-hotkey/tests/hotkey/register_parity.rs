//! Differential tests execute the extracted C wrappers, not a second policy model.
//! No exclusions or intentional behavior differences. Owner stand-ins only model
//! private storage; the unchanged C characterization exercises real ownership,
//! saved bindings, callback dispatch, context teardown and weak expiration.
use obs_hotkey::ffi::pair::obs_hotkey_t;
use obs_hotkey::ffi::register::{self as rs, Callback};
use obs_hotkey::register::Kind;
use proptest::prelude::*;
use std::cell::RefCell;
use std::ffi::{CString, c_char, c_uint, c_void};

type Frontend = unsafe extern "C" fn(*const c_char, *const c_char, Callback, *mut c_void) -> usize;
type Owned =
    unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char, Callback, *mut c_void) -> usize;
unsafe extern "C" {
    fn oracle_obs_hotkey_register_frontend(
        name: *const c_char,
        description: *const c_char,
        callback: Callback,
        data: *mut c_void,
    ) -> usize;
    fn oracle_obs_hotkey_register_source(
        owner: *mut c_void,
        name: *const c_char,
        description: *const c_char,
        callback: Callback,
        data: *mut c_void,
    ) -> usize;
    fn oracle_obs_hotkey_register_output(
        owner: *mut c_void,
        name: *const c_char,
        description: *const c_char,
        callback: Callback,
        data: *mut c_void,
    ) -> usize;
    fn oracle_obs_hotkey_register_encoder(
        owner: *mut c_void,
        name: *const c_char,
        description: *const c_char,
        callback: Callback,
        data: *mut c_void,
    ) -> usize;
    fn oracle_obs_hotkey_register_service(
        owner: *mut c_void,
        name: *const c_char,
        description: *const c_char,
        callback: Callback,
        data: *mut c_void,
    ) -> usize;
    fn oracle_hotkey_register_owner_new(private: bool, token: usize) -> *mut c_void;
    fn oracle_hotkey_register_owner_free(owner: *mut c_void);
    fn oracle_hotkey_register_owner_private(owner: *const c_void) -> bool;
    fn oracle_hotkey_register_owner_context(owner: *mut c_void) -> *mut c_void;
    fn oracle_hotkey_register_owner_token(owner: *const c_void) -> usize;
    fn oracle_hotkey_register_constants(out: *mut usize);
}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
struct State {
    available: bool,
    next: usize,
    events: Vec<Vec<usize>>,
}
thread_local! { static STATE: RefCell<State> = RefCell::default(); }
fn event(values: Vec<usize>) {
    STATE.with_borrow_mut(|s| s.events.push(values));
}

#[unsafe(no_mangle)]
extern "C" fn obs_hotkey_register_lock() -> bool {
    event(vec![0]);
    STATE.with_borrow(|s| s.available)
}
#[unsafe(no_mangle)]
extern "C" fn obs_hotkey_register_unlock() {
    event(vec![1]);
}
#[unsafe(no_mangle)]
unsafe extern "C" fn obs_hotkey_register_private(owner: *mut c_void) -> bool {
    assert!(!owner.is_null());
    // SAFETY: tests pass a live oracle-owned object; NULL must short circuit.
    unsafe { oracle_hotkey_register_owner_private(owner) }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn obs_hotkey_register_weak(kind: c_uint, owner: *mut c_void) -> *mut c_void {
    assert!(!owner.is_null());
    // SAFETY: live oracle-owned object, never a fabricated layout.
    let token = unsafe { oracle_hotkey_register_owner_token(owner) };
    event(vec![2, kind as usize, owner as usize, token]);
    // The stand-in weak handle is compared and forwarded, never dereferenced.
    std::ptr::without_provenance_mut(token)
}
#[unsafe(no_mangle)]
unsafe extern "C" fn obs_hotkey_register_context(_: c_uint, owner: *mut c_void) -> *mut c_void {
    assert!(!owner.is_null());
    // SAFETY: live oracle-owned object of the requested type.
    unsafe { oracle_hotkey_register_owner_context(owner) }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn obs_hotkey_register_insert(
    kind: c_uint,
    weak: *mut c_void,
    context: *mut c_void,
    name: *const c_char,
    description: *const c_char,
    callback: Callback,
    data: *mut c_void,
) -> usize {
    event(vec![
        3,
        kind as usize,
        weak as usize,
        context as usize,
        name as usize,
        description as usize,
        callback.map_or(0, |f| f as usize),
        data as usize,
    ]);
    let id = STATE.with_borrow_mut(|s| {
        let id = s.next;
        s.next = s.next.wrapping_add(1);
        id
    });
    if let Some(callback) = callback {
        // SAFETY: the test callback only records its arguments, including the
        // opaque key; this seam probes exact callback and data forwarding.
        unsafe {
            callback(data, id, std::ptr::without_provenance_mut(123), true);
            callback(data, id, std::ptr::without_provenance_mut(123), false);
        }
    }
    id
}
unsafe extern "C" fn callback(data: *mut c_void, id: usize, key: *mut obs_hotkey_t, pressed: bool) {
    event(vec![
        4,
        data as usize,
        id,
        key as usize,
        usize::from(pressed),
    ]);
}

struct Owner(*mut c_void);
impl Drop for Owner {
    fn drop(&mut self) {
        // SAFETY: exactly one free for the oracle allocation this guard owns.
        unsafe { oracle_hotkey_register_owner_free(self.0) }
    }
}

#[allow(clippy::too_many_arguments)] // Independent generated inputs stay visible in the property test.
fn compare(
    available: bool,
    private: bool,
    null_owner: bool,
    next: usize,
    token: usize,
    steps: &[(u8, bool)],
    name: Vec<u8>,
    description: Vec<u8>,
    null_strings: bool,
) {
    let name = CString::new(name).unwrap();
    let description = CString::new(description).unwrap();
    // SAFETY: allocates an object with the oracle's own C layout.
    let owner = Owner(unsafe { oracle_hotkey_register_owner_new(private, token) });
    assert!(!owner.0.is_null());
    let owner_ptr = if null_owner {
        std::ptr::null_mut()
    } else {
        owner.0
    };
    let name_ptr = if null_strings {
        std::ptr::null()
    } else {
        name.as_ptr()
    };
    let description_ptr = if null_strings {
        std::ptr::null()
    } else {
        description.as_ptr()
    };
    let c_owned: [Owned; 4] = [
        oracle_obs_hotkey_register_source,
        oracle_obs_hotkey_register_output,
        oracle_obs_hotkey_register_encoder,
        oracle_obs_hotkey_register_service,
    ];
    let r_owned: [Owned; 4] = [
        rs::obs_hotkey_register_source,
        rs::obs_hotkey_register_output,
        rs::obs_hotkey_register_encoder,
        rs::obs_hotkey_register_service,
    ];
    let run = |frontend: Frontend, owned: [Owned; 4]| {
        STATE.with_borrow_mut(|s| {
            *s = State {
                available,
                next,
                events: vec![],
            }
        });
        let mut ids = vec![];
        for &(kind, has_callback) in steps {
            let callback: Callback = has_callback.then_some(callback);
            let data = std::ptr::without_provenance_mut(token);
            // SAFETY: C strings remain live, owners are live or NULL; the
            // callback records only opaque pointers. Both implementations use
            // the same instrumented real-dependency boundary.
            ids.push(unsafe {
                if kind == 0 {
                    frontend(name_ptr, description_ptr, callback, data)
                } else {
                    owned[usize::from(kind - 1)](
                        owner_ptr,
                        name_ptr,
                        description_ptr,
                        callback,
                        data,
                    )
                }
            });
        }
        (ids, STATE.with_borrow(Clone::clone))
    };
    assert_eq!(
        run(oracle_obs_hotkey_register_frontend, c_owned),
        run(rs::obs_hotkey_register_frontend, r_owned)
    );
}

#[test]
fn register_characterized_edges_and_abi_constants() {
    let mut values = [0; 6];
    // SAFETY: six writable size_t values, as required by the oracle.
    unsafe { oracle_hotkey_register_constants(values.as_mut_ptr()) };
    assert_eq!(
        values,
        [
            usize::MAX,
            Kind::Frontend as usize,
            Kind::Source as usize,
            Kind::Output as usize,
            Kind::Encoder as usize,
            Kind::Service as usize
        ]
    );
    for available in [false, true] {
        for private in [false, true] {
            for null_owner in [false, true] {
                for next in [0, 1, usize::MAX - 1, usize::MAX] {
                    compare(
                        available,
                        private,
                        null_owner,
                        next,
                        1234,
                        &[
                            (0, true),
                            (1, true),
                            (2, true),
                            (3, true),
                            (4, true),
                            (0, false),
                            (1, false),
                        ],
                        vec![],
                        vec![],
                        false,
                    );
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn register_randomized_differential(available in any::<bool>(), private in any::<bool>(), null_owner in any::<bool>(),
        next in any::<usize>(), token in any::<usize>(), steps in prop::collection::vec((0u8..5, any::<bool>()), 0..40),
        name in prop::collection::vec(1u8..=255, 0..32), description in prop::collection::vec(1u8..=255, 0..32), null_strings in any::<bool>()) {
        compare(available, private, null_owner, next, token, &steps, name, description, null_strings);
    }
}
