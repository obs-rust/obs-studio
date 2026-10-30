//! Tier 3: `signal_handler_*` against the C oracle (oracle/signal.c, the
//! original signal.c.h on the decl oracle).
//!
//! A proptest builds a table of callbacks with actions (none,
//! `signal_handler_remove_current`, disconnect or connect another
//! callback, connect another callback then remove, a nested signal, a
//! nested signal then remove), global callbacks with actions (none,
//! remove, disconnect itself, connect or disconnect another global
//! callback, connect another then remove, a nested signal, a nested
//! signal then remove), and a list of operations
//! (add, connect, connect_ref, disconnect, connect_global,
//! disconnect_global, signal). Both sides run the same script. The test
//! compares every callback call in order (callback id, and the signal name
//! for global callbacks) and every logged line.
//!
//! Excluded: the handler is never destroyed from a callback (C would free
//! it under the running pass).

use std::cell::{Cell, RefCell};
use std::ffi::{CString, c_char, c_int, c_void};

use obs_c_oracle::decl as log_oracle;
use obs_c_oracle::signal as oracle;
use obs_c_oracle::signal::{GlobalSignalCallback, SignalCallback};
use obs_callback::ffi::signal as rust;
use proptest::prelude::*;

const SIGNALS: [&str; 3] = ["a", "b", "c"];
const DECLS: [&str; 4] = ["void a(int n)", "void b()", "void c(ptr p)", "void d()"];
const EXTRA_DECLS: [&str; 4] = ["void a()", "void (int n)", "void e(foo x)", "void d()"];
const CALLBACKS: usize = 8;
const GLOBALS: usize = 3;
const MAX_DEPTH: u32 = 3;

#[derive(Clone, Copy, Debug)]
enum Action {
    None,
    RemoveCurrent,
    DisconnectOther(usize, usize),
    ConnectOther(usize, usize),
    ConnectThenRemove(usize, usize),
    Nested(usize),
    NestedThenRemove(usize),
}

#[derive(Clone, Copy, Debug)]
enum GlobalAction {
    None,
    RemoveCurrent,
    DisconnectSelf,
    ConnectOther(usize),
    DisconnectOther(usize),
    ConnectThenRemove(usize),
    Nested(usize),
    NestedThenRemove(usize),
}

#[derive(Clone, Copy, Debug)]
enum Op {
    Add(usize),
    /// Signal index 3 is an unknown name.
    Connect(usize, usize, bool),
    Disconnect(usize, usize),
    ConnectGlobal(usize),
    DisconnectGlobal(usize),
    Signal(usize),
}

type Handler = *mut c_void;

struct Side {
    create: unsafe extern "C" fn() -> Handler,
    destroy: unsafe extern "C" fn(Handler),
    add: unsafe extern "C" fn(Handler, *const c_char) -> bool,
    connect: unsafe extern "C" fn(Handler, *const c_char, SignalCallback, *mut c_void),
    connect_ref: unsafe extern "C" fn(Handler, *const c_char, SignalCallback, *mut c_void),
    disconnect: unsafe extern "C" fn(Handler, *const c_char, SignalCallback, *mut c_void),
    connect_global: unsafe extern "C" fn(Handler, GlobalSignalCallback, *mut c_void),
    disconnect_global: unsafe extern "C" fn(Handler, GlobalSignalCallback, *mut c_void),
    remove_current: unsafe extern "C" fn(),
    signal: unsafe extern "C" fn(Handler, *const c_char, *mut c_void),
}

const C_SIDE: Side = Side {
    create: oracle::oracle_signal_handler_create,
    destroy: oracle::oracle_signal_handler_destroy,
    add: oracle::oracle_signal_handler_add,
    connect: oracle::oracle_signal_handler_connect,
    connect_ref: oracle::oracle_signal_handler_connect_ref,
    disconnect: oracle::oracle_signal_handler_disconnect,
    connect_global: oracle::oracle_signal_handler_connect_global,
    disconnect_global: oracle::oracle_signal_handler_disconnect_global,
    remove_current: oracle::oracle_signal_handler_remove_current,
    signal: oracle::oracle_signal_handler_signal,
};

// Adapters from the oracle's `void *` signatures to the Rust shim's types.
// The callback types differ only in a pointer parameter type.

unsafe extern "C" fn r_create() -> Handler {
    rust::signal_handler_create().cast()
}
unsafe extern "C" fn r_destroy(h: Handler) {
    // SAFETY: forwarded.
    unsafe { rust::signal_handler_destroy(h.cast()) }
}
unsafe extern "C" fn r_add(h: Handler, d: *const c_char) -> bool {
    // SAFETY: forwarded.
    unsafe { rust::signal_handler_add(h.cast(), d) }
}
unsafe extern "C" fn r_connect(h: Handler, s: *const c_char, cb: SignalCallback, d: *mut c_void) {
    // SAFETY: forwarded; the callback types have the same ABI.
    unsafe {
        rust::signal_handler_connect(
            h.cast(),
            s,
            std::mem::transmute::<SignalCallback, rust::signal_callback_t>(cb),
            d,
        )
    }
}
unsafe extern "C" fn r_connect_ref(
    h: Handler,
    s: *const c_char,
    cb: SignalCallback,
    d: *mut c_void,
) {
    // SAFETY: as r_connect.
    unsafe {
        rust::signal_handler_connect_ref(
            h.cast(),
            s,
            std::mem::transmute::<SignalCallback, rust::signal_callback_t>(cb),
            d,
        )
    }
}
unsafe extern "C" fn r_disconnect(
    h: Handler,
    s: *const c_char,
    cb: SignalCallback,
    d: *mut c_void,
) {
    // SAFETY: as r_connect.
    unsafe {
        rust::signal_handler_disconnect(
            h.cast(),
            s,
            std::mem::transmute::<SignalCallback, rust::signal_callback_t>(cb),
            d,
        )
    }
}
unsafe extern "C" fn r_connect_global(h: Handler, cb: GlobalSignalCallback, d: *mut c_void) {
    // SAFETY: as r_connect.
    unsafe {
        rust::signal_handler_connect_global(
            h.cast(),
            std::mem::transmute::<GlobalSignalCallback, rust::global_signal_callback_t>(cb),
            d,
        )
    }
}
unsafe extern "C" fn r_disconnect_global(h: Handler, cb: GlobalSignalCallback, d: *mut c_void) {
    // SAFETY: as r_connect.
    unsafe {
        rust::signal_handler_disconnect_global(
            h.cast(),
            std::mem::transmute::<GlobalSignalCallback, rust::global_signal_callback_t>(cb),
            d,
        )
    }
}
unsafe extern "C" fn r_remove_current() {
    rust::signal_handler_remove_current();
}
unsafe extern "C" fn r_signal(h: Handler, s: *const c_char, p: *mut c_void) {
    // SAFETY: forwarded.
    unsafe { rust::signal_handler_signal(h.cast(), s, p.cast()) }
}

const RUST_SIDE: Side = Side {
    create: r_create,
    destroy: r_destroy,
    add: r_add,
    connect: r_connect,
    connect_ref: r_connect_ref,
    disconnect: r_disconnect,
    connect_global: r_connect_global,
    disconnect_global: r_disconnect_global,
    remove_current: r_remove_current,
    signal: r_signal,
};

/// One script run: the side, its handler, the callback table.
struct Run {
    side: &'static Side,
    handler: Cell<Handler>,
    names: Vec<CString>,
    callbacks: Vec<Ctx>,
    globals: Vec<GlobalCtx>,
}

struct Ctx {
    id: usize,
    action: Action,
    run: Cell<*const Run>,
}

struct GlobalCtx {
    id: usize,
    action: GlobalAction,
    run: Cell<*const Run>,
}

thread_local! {
    static EVENTS: RefCell<Vec<(usize, Option<String>)>> = const { RefCell::new(Vec::new()) };
    static DEPTH: Cell<u32> = const { Cell::new(0) };
}

impl Run {
    fn name(&self, sig: usize) -> *const c_char {
        self.names[sig].as_ptr()
    }

    fn data(&self, id: usize) -> *mut c_void {
        (&raw const self.callbacks[id]).cast_mut().cast()
    }

    fn global_data(&self, id: usize) -> *mut c_void {
        (&raw const self.globals[id]).cast_mut().cast()
    }

    fn signal(&self, sig: usize) {
        DEPTH.with(|d| d.set(d.get() + 1));
        // SAFETY: a live handler and a NUL-terminated name; no calldata.
        unsafe { (self.side.signal)(self.handler.get(), self.name(sig), std::ptr::null_mut()) };
        DEPTH.with(|d| d.set(d.get() - 1));
    }
}

unsafe extern "C" fn on_signal(data: *mut c_void, _params: *mut c_void) {
    // SAFETY: every connect passes a `Ctx` of a live `Run`.
    let ctx = unsafe { &*data.cast::<Ctx>() };
    // SAFETY: the run outlives its script.
    let run = unsafe { &*ctx.run.get() };
    EVENTS.with(|e| e.borrow_mut().push((ctx.id, None)));
    let side = run.side;
    let h = run.handler.get();
    let nested_ok = DEPTH.with(|d| d.get()) < MAX_DEPTH;
    // SAFETY: the side's functions on its live handler, names and data.
    unsafe {
        match ctx.action {
            Action::None => {}
            Action::RemoveCurrent => (side.remove_current)(),
            Action::DisconnectOther(sig, id) => {
                (side.disconnect)(h, run.name(sig), Some(on_signal), run.data(id))
            }
            Action::ConnectOther(sig, id) => {
                (side.connect)(h, run.name(sig), Some(on_signal), run.data(id))
            }
            Action::ConnectThenRemove(sig, id) => {
                (side.connect)(h, run.name(sig), Some(on_signal), run.data(id));
                (side.remove_current)();
            }
            Action::Nested(sig) => {
                if nested_ok {
                    run.signal(sig);
                }
            }
            Action::NestedThenRemove(sig) => {
                if nested_ok {
                    run.signal(sig);
                }
                (side.remove_current)();
            }
        }
    }
}

unsafe extern "C" fn on_global(data: *mut c_void, signal: *const c_char, _params: *mut c_void) {
    // SAFETY: every connect_global passes a `GlobalCtx` of a live `Run`.
    let ctx = unsafe { &*data.cast::<GlobalCtx>() };
    // SAFETY: the run outlives its script.
    let run = unsafe { &*ctx.run.get() };
    // SAFETY: the signal name is the caller's NUL-terminated string.
    let name = unsafe { std::ffi::CStr::from_ptr(signal) }
        .to_string_lossy()
        .into_owned();
    EVENTS.with(|e| e.borrow_mut().push((ctx.id, Some(name))));
    // SAFETY: the side's functions on its live handler.
    unsafe {
        match ctx.action {
            GlobalAction::None => {}
            GlobalAction::RemoveCurrent => (run.side.remove_current)(),
            GlobalAction::DisconnectSelf => {
                (run.side.disconnect_global)(run.handler.get(), Some(on_global), data)
            }
            GlobalAction::ConnectOther(id) => {
                (run.side.connect_global)(run.handler.get(), Some(on_global), run.global_data(id))
            }
            GlobalAction::DisconnectOther(id) => (run.side.disconnect_global)(
                run.handler.get(),
                Some(on_global),
                run.global_data(id),
            ),
            GlobalAction::ConnectThenRemove(id) => {
                (run.side.connect_global)(run.handler.get(), Some(on_global), run.global_data(id));
                (run.side.remove_current)();
            }
            GlobalAction::Nested(sig) => {
                if DEPTH.with(|d| d.get()) < MAX_DEPTH {
                    run.signal(sig);
                }
            }
            GlobalAction::NestedThenRemove(sig) => {
                if DEPTH.with(|d| d.get()) < MAX_DEPTH {
                    run.signal(sig);
                }
                (run.side.remove_current)();
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    adds: Vec<bool>,
    events: Vec<(usize, Option<String>)>,
    log: String,
}

fn run_script(
    side: &'static Side,
    actions: &[Action],
    global_actions: &[GlobalAction],
    global_on: &[bool],
    ops: &[Op],
) -> Outcome {
    let _guard = super::LOG_HANDLER.lock().unwrap_or_else(|e| e.into_inner());
    let mut names: Vec<CString> = SIGNALS.iter().map(|s| CString::new(*s).unwrap()).collect();
    names.push(CString::new("nope").unwrap());
    let run = Box::new(Run {
        side,
        handler: Cell::new(std::ptr::null_mut()),
        names,
        callbacks: actions
            .iter()
            .enumerate()
            .map(|(id, &action)| Ctx {
                id,
                action,
                run: Cell::new(std::ptr::null()),
            })
            .collect(),
        globals: global_actions
            .iter()
            .enumerate()
            .map(|(id, &action)| GlobalCtx {
                id: 100 + id,
                action,
                run: Cell::new(std::ptr::null()),
            })
            .collect(),
    });
    let run_ptr: *const Run = &*run;
    for c in &run.callbacks {
        c.run.set(run_ptr);
    }
    for g in &run.globals {
        g.run.set(run_ptr);
    }
    EVENTS.with(|e| e.borrow_mut().clear());

    let decls: Vec<CString> = DECLS
        .iter()
        .chain(EXTRA_DECLS.iter())
        .map(|d| CString::new(*d).unwrap())
        .collect();
    let mut adds = Vec::new();
    let mut log = vec![0u8; 16384];
    let mut level: c_int = 0;
    // SAFETY: the side's functions on its own handler, with live names,
    // declarations and callback data. The log lock is held.
    let len = unsafe {
        log_oracle::oracle_decl_log_begin_all();
        run.handler.set((side.create)());
        let h = run.handler.get();
        for d in &decls[..3] {
            adds.push((side.add)(h, d.as_ptr()));
        }
        for (id, &on) in global_on.iter().enumerate() {
            if on {
                (side.connect_global)(h, Some(on_global), run.global_data(id));
            }
        }
        for op in ops {
            match *op {
                Op::Add(i) => adds.push((side.add)(h, decls[3 + i].as_ptr())),
                Op::Connect(sig, id, false) => {
                    (side.connect)(h, run.name(sig), Some(on_signal), run.data(id))
                }
                Op::Connect(sig, id, true) => {
                    (side.connect_ref)(h, run.name(sig), Some(on_signal), run.data(id))
                }
                Op::Disconnect(sig, id) => {
                    (side.disconnect)(h, run.name(sig), Some(on_signal), run.data(id))
                }
                Op::ConnectGlobal(id) => {
                    (side.connect_global)(h, Some(on_global), run.global_data(id))
                }
                Op::DisconnectGlobal(id) => {
                    (side.disconnect_global)(h, Some(on_global), run.global_data(id))
                }
                Op::Signal(sig) => run.signal(sig),
            }
        }
        // The creator's reference. keep_ref callbacks left connected keep
        // the handler alive (a leak on both sides, as in C).
        (side.destroy)(h);
        log_oracle::oracle_decl_log_end(log.as_mut_ptr().cast(), log.len(), &mut level)
    };
    log.truncate(len);
    Outcome {
        adds,
        events: EVENTS.with(|e| e.borrow_mut().split_off(0)),
        log: String::from_utf8_lossy(&log).into_owned(),
    }
}

fn assert_parity(
    actions: &[Action],
    global_actions: &[GlobalAction],
    global_on: &[bool],
    ops: &[Op],
) {
    let c_side = run_script(&C_SIDE, actions, global_actions, global_on, ops);
    let rust_side = run_script(&RUST_SIDE, actions, global_actions, global_on, ops);
    assert_eq!(
        rust_side, c_side,
        "actions {actions:?} globals {global_actions:?} {global_on:?} ops {ops:?}"
    );
}

#[test]
fn signal_basic_script_matches_oracle() {
    use Action as A;
    use Op::*;
    let actions = [
        A::None,
        A::RemoveCurrent,
        A::DisconnectOther(0, 0),
        A::ConnectOther(0, 4),
        A::NestedThenRemove(1),
        A::Nested(2),
        A::None,
        A::None,
    ];
    let globals = [
        GlobalAction::None,
        GlobalAction::RemoveCurrent,
        GlobalAction::DisconnectSelf,
    ];
    let ops = [
        Connect(0, 0, false),
        Connect(0, 1, false),
        Connect(0, 2, false),
        Connect(0, 3, true),
        Connect(1, 6, false),
        Connect(0, 4, false),
        Connect(2, 7, true),
        Connect(1, 5, false),
        Connect(3, 0, false),
        Signal(0),
        Signal(0),
        Signal(1),
        Signal(3),
        Disconnect(0, 3),
        Signal(0),
        Add(0),
        Add(1),
        Add(2),
        Add(3),
        Signal(2),
    ];
    assert_parity(&actions, &globals, &[true, true, true], &ops);
}

fn action() -> impl Strategy<Value = Action> {
    let sig = 0..SIGNALS.len();
    let id = 0..CALLBACKS;
    prop_oneof![
        3 => Just(Action::None),
        2 => Just(Action::RemoveCurrent),
        2 => (sig.clone(), id.clone()).prop_map(|(s, i)| Action::DisconnectOther(s, i)),
        2 => (sig.clone(), id.clone()).prop_map(|(s, i)| Action::ConnectOther(s, i)),
        1 => (sig.clone(), id).prop_map(|(s, i)| Action::ConnectThenRemove(s, i)),
        1 => sig.clone().prop_map(Action::Nested),
        1 => sig.prop_map(Action::NestedThenRemove),
    ]
}

fn global_action() -> impl Strategy<Value = GlobalAction> {
    prop_oneof![
        Just(GlobalAction::None),
        Just(GlobalAction::RemoveCurrent),
        Just(GlobalAction::DisconnectSelf),
        (0..GLOBALS).prop_map(GlobalAction::ConnectOther),
        (0..GLOBALS).prop_map(GlobalAction::DisconnectOther),
        (0..GLOBALS).prop_map(GlobalAction::ConnectThenRemove),
        (0..SIGNALS.len()).prop_map(GlobalAction::Nested),
        (0..SIGNALS.len()).prop_map(GlobalAction::NestedThenRemove),
    ]
}

fn op() -> impl Strategy<Value = Op> {
    let sig = 0..SIGNALS.len() + 1;
    let id = 0..CALLBACKS;
    prop_oneof![
        1 => (0..EXTRA_DECLS.len()).prop_map(Op::Add),
        5 => (sig.clone(), id.clone(), any::<bool>()).prop_map(|(s, i, k)| Op::Connect(s, i, k)),
        2 => (sig.clone(), id).prop_map(|(s, i)| Op::Disconnect(s, i)),
        1 => (0..GLOBALS).prop_map(Op::ConnectGlobal),
        1 => (0..GLOBALS).prop_map(Op::DisconnectGlobal),
        4 => sig.prop_map(Op::Signal),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]

    #[test]
    fn random_scripts_match_oracle(
        actions in prop::collection::vec(action(), CALLBACKS),
        globals in prop::collection::vec(global_action(), GLOBALS),
        global_on in prop::collection::vec(any::<bool>(), GLOBALS),
        ops in prop::collection::vec(op(), 0..40),
    ) {
        assert_parity(&actions, &globals, &global_on, &ops);
    }
}
