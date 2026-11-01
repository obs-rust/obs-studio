//! Tier 3: the proc shims match `libobs/callback/proc.c.h`.
//!
//! Both sides call the real `parse_decl_string` and the real `blog`. The
//! handler object differs: C uses `bmalloc`, the shim uses a `Box`. Each
//! side's `bnum_allocs` still returns to where it started, because the
//! declaration strings are freed with `bfree`.
//!
//! The duplicate-name warning is logged after the table lock is released.
//! C logs it while holding the recursive mutex. The first callback is kept
//! either way. `pthread_mutex_init_recursive` failure is not reproduced.
//!
//! A null procedure name once a proc is registered faults in `strcmp`, and
//! a null callback faults when called. Those paths are not called here.
//! The C test does not call them either.
//!
//! The recorder is process-wide. It ignores lines from other threads, so
//! the rest of this binary can blog at the same time, also after a test
//! restores the old handler (a line can still be on its way).
//! `bnum_allocs` is also process-wide. Tests that read it run in their own
//! process.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use std::ffi::{CStr, c_char, c_int, c_long, c_void};
use std::mem::size_of;
use std::ptr;

use obs_c_oracle::proc as oracle;
use obs_callback::ffi::calldata::{calldata, calldata_get_data, calldata_set_data};
use obs_callback::ffi::proc::{
    proc_handler_add, proc_handler_call, proc_handler_create, proc_handler_destroy,
};
use obs_util as _;
use obs_util::base::{LOG_ERROR, LOG_WARNING};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type ProcCb = Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>;
type AddFn = unsafe extern "C" fn(*mut c_void, *const c_char, ProcCb, *mut c_void);
type CallFn = unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_void) -> bool;
type DestroyFn = unsafe extern "C" fn(*mut c_void);

#[repr(C)]
struct Rec {
    calls: c_int,
    level: c_int,
    message: [c_char; 256],
}

impl Rec {
    fn new() -> Self {
        Self {
            calls: 0,
            level: 0,
            message: [0; 256],
        }
    }

    fn text(&self) -> String {
        let bytes: Vec<u8> = self.message.iter().map(|b| *b as u8).collect();
        let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..end]).into_owned()
    }
}

struct Saved {
    handler: *mut c_void,
    param: *mut c_void,
}

impl Saved {
    fn capture() -> Self {
        let mut saved = Self {
            handler: ptr::null_mut(),
            param: ptr::null_mut(),
        };
        // SAFETY: the out-pointers are live locals.
        unsafe { base_get_log_handler(&mut saved.handler, &mut saved.param) };
        saved
    }
}

impl Drop for Saved {
    fn drop(&mut self) {
        // SAFETY: the pointers were read from the same setter. The thread
        // filter stays on: a line another thread is still delivering to
        // the recorder (`blog` calls the handler after dropping the base
        // mutex) must not land in this record once it is gone.
        unsafe { base_set_log_handler(self.handler, self.param) };
    }
}

fn arm(rec: &mut Rec) {
    // SAFETY: `base_test_record` writes one `Rec`. The record outlives the
    // handler. The owner is this thread; other tests blog on their own.
    unsafe {
        base_test_record_only_this_thread();
        base_set_log_handler(base_test_record_ptr(), (rec as *mut Rec).cast());
    }
}

unsafe extern "C" {
    fn bnum_allocs() -> c_long;
    fn bfree(ptr: *mut c_void);
    fn base_test_record_ptr() -> *mut c_void;
    fn base_test_record_only_this_thread();
    fn base_log_literal(handler: *mut c_void, param: *mut c_void, level: c_int, msg: *const c_char);
    fn blog(level: c_int, format: *const c_char, ...);
}

fn allocs() -> c_long {
    unsafe { bnum_allocs() }
}

struct Stack(calldata);

impl Stack {
    fn new() -> Self {
        Self(calldata {
            stack: ptr::null_mut(),
            size: 0,
            capacity: 0,
            fixed: false,
        })
    }

    fn set_int(&mut self, name: &CStr, value: i64) {
        unsafe {
            calldata_set_data(
                &mut self.0,
                name.as_ptr(),
                (&value as *const i64).cast(),
                size_of::<i64>(),
            );
        }
    }
}

impl Drop for Stack {
    fn drop(&mut self) {
        if !self.0.fixed && !self.0.stack.is_null() {
            unsafe { bfree(self.0.stack.cast()) };
        }
    }
}

#[repr(C)]
struct Seen {
    hits: i32,
    data: usize,
    params: usize,
    n: i64,
}

unsafe extern "C" fn count_call(data: *mut c_void, params: *mut c_void) {
    let seen = unsafe { &mut *data.cast::<Seen>() };
    seen.hits += 1;
    seen.data = data as usize;
    seen.params = params as usize;
    if !params.is_null() {
        let mut n = 0i64;
        unsafe {
            calldata_get_data(
                params.cast(),
                c"n".as_ptr(),
                (&mut n as *mut i64).cast(),
                size_of::<i64>(),
            );
        }
        seen.n = n;
    }
}

struct Bridge {
    add: AddFn,
    call: CallFn,
    handler: *mut c_void,
    hits: i32,
}

unsafe extern "C" fn inner_call(data: *mut c_void, _params: *mut c_void) {
    unsafe { (*data.cast::<Bridge>()).hits += 10 };
}

unsafe extern "C" fn mark_replaced(data: *mut c_void, _params: *mut c_void) {
    unsafe { (*data.cast::<Bridge>()).hits += 1 };
}

unsafe extern "C" fn outer_call(data: *mut c_void, params: *mut c_void) {
    let seen = unsafe { &mut *data.cast::<Bridge>() };
    seen.hits += 1;
    unsafe {
        (seen.add)(
            seen.handler,
            c"void inner()".as_ptr(),
            Some(inner_call),
            data,
        );
        assert!((seen.call)(seen.handler, c"inner".as_ptr(), params));
        (seen.add)(
            seen.handler,
            c"void inner()".as_ptr(),
            Some(mark_replaced),
            data,
        );
        assert!((seen.call)(seen.handler, c"inner".as_ptr(), params));
    }
}

struct Slot {
    destroy: DestroyFn,
    handler: *mut c_void,
}

unsafe extern "C" fn destroy_from_call(data: *mut c_void, _params: *mut c_void) {
    let slot = unsafe { &mut *data.cast::<Slot>() };
    unsafe { (slot.destroy)(slot.handler) };
    slot.handler = ptr::null_mut();
}

unsafe extern "C" fn rust_add(
    handler: *mut c_void,
    decl: *const c_char,
    proc: ProcCb,
    data: *mut c_void,
) {
    unsafe { proc_handler_add(handler.cast(), decl, proc, data) };
}

unsafe extern "C" fn rust_call(
    handler: *mut c_void,
    name: *const c_char,
    params: *mut c_void,
) -> bool {
    unsafe { proc_handler_call(handler.cast(), name, params) }
}

unsafe extern "C" fn rust_destroy(handler: *mut c_void) {
    unsafe { proc_handler_destroy(handler.cast()) };
}

unsafe extern "C" fn c_add(
    handler: *mut c_void,
    decl: *const c_char,
    proc: ProcCb,
    data: *mut c_void,
) {
    unsafe { oracle::oracle_proc_handler_add(handler, decl, proc, data) };
}

unsafe extern "C" fn c_call(
    handler: *mut c_void,
    name: *const c_char,
    params: *mut c_void,
) -> bool {
    unsafe { oracle::oracle_proc_handler_call(handler, name, params) }
}

unsafe extern "C" fn c_destroy(handler: *mut c_void) {
    unsafe { oracle::oracle_proc_handler_destroy(handler) };
}

struct Side {
    create: unsafe extern "C" fn() -> *mut c_void,
    destroy: DestroyFn,
    add: AddFn,
    call: CallFn,
}

fn rust_side() -> Side {
    Side {
        create: rust_create,
        destroy: rust_destroy,
        add: rust_add,
        call: rust_call,
    }
}

fn c_side() -> Side {
    Side {
        create: c_create,
        destroy: c_destroy,
        add: c_add,
        call: c_call,
    }
}

unsafe extern "C" fn rust_create() -> *mut c_void {
    unsafe { proc_handler_create().cast() }
}

unsafe extern "C" fn c_create() -> *mut c_void {
    unsafe { oracle::oracle_proc_handler_create() }
}

/// Every test here swaps the process-wide log handler, so it holds the
/// lock shared with `decl_parity` (see `main.rs`).
fn lock() -> std::sync::MutexGuard<'static, ()> {
    super::LOG_HANDLER
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}

/// Runs `test` alone in a child process. `bnum_allocs` moves when any
/// other test in this binary allocates, so a delta taken in the parent
/// races (#1133). Returns true in the child, where the caller measures.
fn process_isolated(test: &str) -> bool {
    const MARKER: &str = "OBS_CALLBACK_ISOLATED_TEST";
    if std::env::var(MARKER).as_deref() == Ok(test) {
        return true;
    }
    let exe = std::env::current_exe().expect("path of the test binary");
    let out = std::process::Command::new(exe)
        .args([test, "--exact", "--test-threads=1", "--nocapture"])
        .env(MARKER, test)
        .output()
        .expect("start the isolated test process");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "isolated {test} failed:\n{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains("1 passed"),
        "isolated {test} did not run exactly one test:\n{stdout}\n{stderr}"
    );
    false
}

fn null_handler(side: &Side) {
    let before = allocs();
    let mut seen = Seen {
        hits: 0,
        data: 0,
        params: 0,
        n: 0,
    };
    unsafe {
        (side.destroy)(ptr::null_mut());
        (side.add)(
            ptr::null_mut(),
            c"void f()".as_ptr(),
            Some(count_call),
            (&mut seen as *mut Seen).cast(),
        );
        assert!(!(side.call)(
            ptr::null_mut(),
            c"f".as_ptr(),
            ptr::null_mut()
        ));
    }
    assert_eq!(seen.hits, 0);
    assert_eq!(allocs(), before);
}

fn create_destroy(side: &Side) {
    let before = allocs();
    let handler = unsafe { (side.create)() };
    assert!(!handler.is_null());
    unsafe { (side.destroy)(handler) };
    assert_eq!(allocs(), before);
}

fn call_passes_params(side: &Side) -> Rec {
    let before = allocs();
    let mut rec = Rec::new();
    let _saved = Saved::capture();
    arm(&mut rec);
    let handler = unsafe { (side.create)() };
    let mut seen = Seen {
        hits: 0,
        data: 0,
        params: 0,
        n: 0,
    };
    let mut stack = Stack::new();
    stack.set_int(c"n", 7);
    let seen_ptr = (&mut seen as *mut Seen).cast();
    unsafe {
        (side.add)(
            handler,
            c"void f(in int n)".as_ptr(),
            Some(count_call),
            seen_ptr,
        );
        assert_eq!(rec.calls, 0);
        assert!((side.call)(
            handler,
            c"f".as_ptr(),
            (&mut stack.0 as *mut calldata).cast()
        ));
    }
    assert_eq!(seen.hits, 1);
    assert_eq!(seen.data, seen_ptr as usize);
    assert_eq!(seen.params, &stack.0 as *const calldata as usize);
    assert_eq!(seen.n, 7);

    seen = Seen {
        hits: 0,
        data: 0,
        params: 0,
        n: 0,
    };
    let seen_ptr = (&mut seen as *mut Seen).cast();
    unsafe {
        assert!((side.call)(handler, c"f".as_ptr(), ptr::null_mut()));
    }
    assert_eq!(seen.hits, 1);
    assert_eq!(seen.params, 0);
    assert_eq!(rec.calls, 0);

    unsafe {
        (side.add)(
            handler,
            c"void  spaced()".as_ptr(),
            Some(count_call),
            seen_ptr,
        );
        assert!((side.call)(
            handler,
            c"spaced".as_ptr(),
            (&mut stack.0 as *mut calldata).cast()
        ));
        assert!(!(side.call)(
            handler,
            c"void  spaced()".as_ptr(),
            ptr::null_mut()
        ));
    }
    assert_eq!(seen.hits, 2);
    drop(stack);
    unsafe { (side.destroy)(handler) };
    assert_eq!(allocs(), before);
    rec
}

fn unknown_name(side: &Side) -> Rec {
    let before = allocs();
    let mut rec = Rec::new();
    let _saved = Saved::capture();
    arm(&mut rec);
    let handler = unsafe { (side.create)() };
    unsafe {
        assert!(!(side.call)(handler, ptr::null(), ptr::null_mut()));
        assert!(!(side.call)(handler, c"".as_ptr(), ptr::null_mut()));
        assert!(!(side.call)(handler, c"missing".as_ptr(), ptr::null_mut()));
    }
    assert_eq!(rec.calls, 0);

    let mut lower = Seen {
        hits: 0,
        data: 0,
        params: 0,
        n: 0,
    };
    let mut longer = Seen {
        hits: 0,
        data: 0,
        params: 0,
        n: 0,
    };
    unsafe {
        (side.add)(
            handler,
            c"void foo()".as_ptr(),
            Some(count_call),
            (&mut lower as *mut Seen).cast(),
        );
        (side.add)(
            handler,
            c"void foobar()".as_ptr(),
            Some(count_call),
            (&mut longer as *mut Seen).cast(),
        );
        (side.add)(
            handler,
            c"void Foo()".as_ptr(),
            Some(count_call),
            (&mut lower as *mut Seen).cast(),
        );
        assert!((side.call)(handler, c"foo".as_ptr(), ptr::null_mut()));
        assert_eq!(lower.hits, 1);
        assert_eq!(longer.hits, 0);
        assert!((side.call)(handler, c"foobar".as_ptr(), ptr::null_mut()));
        assert_eq!(longer.hits, 1);
        assert!((side.call)(handler, c"Foo".as_ptr(), ptr::null_mut()));
        assert_eq!(lower.hits, 2);
        assert!(!(side.call)(handler, c"FOO".as_ptr(), ptr::null_mut()));
        assert!(!(side.call)(handler, c"fo".as_ptr(), ptr::null_mut()));
        (side.destroy)(handler);
    }
    assert_eq!(rec.calls, 0);
    assert_eq!(allocs(), before);
    rec
}

fn add_invalid(side: &Side, decl: *const c_char, call_name: *const c_char) -> (Rec, i32) {
    let handler = unsafe { (side.create)() };
    let mut rec = Rec::new();
    let _saved = Saved::capture();
    arm(&mut rec);
    let mut seen = Seen {
        hits: 0,
        data: 0,
        params: 0,
        n: 0,
    };
    unsafe {
        (side.add)(
            handler,
            decl,
            Some(count_call),
            (&mut seen as *mut Seen).cast(),
        );
        if !call_name.is_null() {
            assert!(!(side.call)(handler, call_name, ptr::null_mut()));
        }
        (side.destroy)(handler);
    }
    (rec, seen.hits)
}

fn invalid_decl(side: &Side) -> (c_long, i32) {
    let before = allocs();
    let (bad, hits) = add_invalid(side, c"void f(foo a)".as_ptr(), c"f".as_ptr());
    assert!(bad.calls >= 1);
    assert_eq!(bad.level, LOG_ERROR);
    assert_eq!(bad.text(), "Function declaration invalid: void f(foo a)");
    assert_eq!(hits, 0);

    let (empty, _) = add_invalid(side, c"".as_ptr(), c"".as_ptr());
    assert_eq!(empty.level, LOG_ERROR);
    assert_eq!(empty.text(), "Function declaration invalid: ");

    let (null_decl, null_hits) = add_invalid(side, ptr::null(), ptr::null());
    assert_eq!(null_decl.level, LOG_ERROR);
    assert_eq!(null_decl.text(), "Function declaration invalid: (null)");
    assert_eq!(null_hits, 0);

    let handler = unsafe { (side.create)() };
    let mut seen = Seen {
        hits: 0,
        data: 0,
        params: 0,
        n: 0,
    };
    unsafe {
        (side.add)(
            handler,
            c"void f()".as_ptr(),
            Some(count_call),
            (&mut seen as *mut Seen).cast(),
        );
        assert!((side.call)(handler, c"f".as_ptr(), ptr::null_mut()));
        (side.destroy)(handler);
    }
    assert_eq!(seen.hits, 1);
    assert_eq!(allocs(), before);
    (before, seen.hits)
}

struct Which {
    value: i32,
}

unsafe extern "C" fn mark_first(data: *mut c_void, _params: *mut c_void) {
    unsafe { (*data.cast::<Which>()).value = 1 };
}

unsafe extern "C" fn mark_second(data: *mut c_void, _params: *mut c_void) {
    unsafe { (*data.cast::<Which>()).value = 2 };
}

unsafe extern "C" fn bump_seen(data: *mut c_void, _params: *mut c_void) {
    unsafe { (*data.cast::<Seen>()).hits += 50 };
}

fn duplicate_name(side: &Side) -> Rec {
    let before = allocs();
    let mut rec = Rec::new();
    let _saved = Saved::capture();
    arm(&mut rec);
    let handler = unsafe { (side.create)() };
    let mut which = Which { value: 0 };
    unsafe {
        (side.add)(
            handler,
            c"void f()".as_ptr(),
            Some(mark_first),
            (&mut which as *mut Which).cast(),
        );
        (side.add)(
            handler,
            c"int f(out string s)".as_ptr(),
            Some(mark_second),
            (&mut which as *mut Which).cast(),
        );
        assert!((side.call)(handler, c"f".as_ptr(), ptr::null_mut()));
        (side.destroy)(handler);
    }
    assert_eq!(rec.calls, 1);
    assert_eq!(rec.level, LOG_WARNING);
    assert_eq!(rec.text(), "Procedure 'f' already exists");
    assert_eq!(which.value, 1);
    assert_eq!(allocs(), before);
    rec
}

fn reenter(side: &Side) -> Rec {
    let before = allocs();
    let mut rec = Rec::new();
    let _saved = Saved::capture();
    arm(&mut rec);
    let handler = unsafe { (side.create)() };
    let mut bridge = Bridge {
        add: side.add,
        call: side.call,
        handler,
        hits: 0,
    };
    unsafe {
        (side.add)(
            handler,
            c"void outer()".as_ptr(),
            Some(outer_call),
            (&mut bridge as *mut Bridge).cast(),
        );
        assert!((side.call)(handler, c"outer".as_ptr(), ptr::null_mut()));
        (side.destroy)(handler);
    }
    assert_eq!(bridge.hits, 21);
    assert_eq!(rec.calls, 1);
    assert_eq!(rec.level, LOG_WARNING);
    assert_eq!(rec.text(), "Procedure 'inner' already exists");
    assert_eq!(allocs(), before);
    rec
}

fn destroy_from_callback(side: &Side) {
    let before = allocs();
    let handler = unsafe { (side.create)() };
    let mut slot = Slot {
        destroy: side.destroy,
        handler,
    };
    unsafe {
        (side.add)(
            handler,
            c"void stop()".as_ptr(),
            Some(destroy_from_call),
            (&mut slot as *mut Slot).cast(),
        );
        assert!((side.call)(handler, c"stop".as_ptr(), ptr::null_mut()));
    }
    assert!(slot.handler.is_null());
    assert_eq!(allocs(), before);
}

fn null_proc(side: &Side) {
    let before = allocs();
    let handler = unsafe { (side.create)() };
    unsafe {
        (side.add)(handler, c"void none()".as_ptr(), None, ptr::null_mut());
        (side.destroy)(handler);
    }
    assert_eq!(allocs(), before);
}

fn same_text(left: &Rec, right: &Rec) {
    assert_eq!(left.calls, right.calls, "log calls");
    assert_eq!(left.level, right.level, "log level");
    assert_eq!(left.text(), right.text(), "log text");
}

#[test]
fn null_handler_test() {
    if !process_isolated("callback::proc_parity::null_handler_test") {
        return;
    }
    let _gate = lock();
    null_handler(&c_side());
    null_handler(&rust_side());
}

#[test]
fn create_destroy_test() {
    if !process_isolated("callback::proc_parity::create_destroy_test") {
        return;
    }
    let _gate = lock();
    create_destroy(&c_side());
    create_destroy(&rust_side());
}

#[test]
fn call_passes_params_test() {
    if !process_isolated("callback::proc_parity::call_passes_params_test") {
        return;
    }
    let _gate = lock();
    let c_rec = call_passes_params(&c_side());
    let rust_rec = call_passes_params(&rust_side());
    same_text(&c_rec, &rust_rec);
}

#[test]
fn unknown_name_test() {
    if !process_isolated("callback::proc_parity::unknown_name_test") {
        return;
    }
    let _gate = lock();
    let c_rec = unknown_name(&c_side());
    let rust_rec = unknown_name(&rust_side());
    same_text(&c_rec, &rust_rec);
}

#[test]
fn invalid_decl_test() {
    if !process_isolated("callback::proc_parity::invalid_decl_test") {
        return;
    }
    let _gate = lock();
    let (c_before, c_hits) = invalid_decl(&c_side());
    let (rust_before, rust_hits) = invalid_decl(&rust_side());
    assert_eq!(c_hits, rust_hits);
    let _ = (c_before, rust_before);
}

#[test]
fn duplicate_name_test() {
    if !process_isolated("callback::proc_parity::duplicate_name_test") {
        return;
    }
    let _gate = lock();
    let c_rec = duplicate_name(&c_side());
    let rust_rec = duplicate_name(&rust_side());
    same_text(&c_rec, &rust_rec);
}

#[test]
fn reenter_test() {
    if !process_isolated("callback::proc_parity::reenter_test") {
        return;
    }
    let _gate = lock();
    let c_rec = reenter(&c_side());
    let rust_rec = reenter(&rust_side());
    same_text(&c_rec, &rust_rec);
}

#[test]
fn destroy_from_callback_test() {
    if !process_isolated("callback::proc_parity::destroy_from_callback_test") {
        return;
    }
    let _gate = lock();
    destroy_from_callback(&c_side());
    destroy_from_callback(&rust_side());
}

#[test]
fn null_proc_test() {
    if !process_isolated("callback::proc_parity::null_proc_test") {
        return;
    }
    let _gate = lock();
    null_proc(&c_side());
    null_proc(&rust_side());
}

#[test]
fn other_thread_log_is_ignored_test() {
    let _gate = lock();
    let _saved = Saved::capture();
    let mut rec = Rec::new();
    arm(&mut rec);
    let logged = std::thread::spawn(|| unsafe {
        blog(LOG_ERROR, c"from another thread".as_ptr());
    });
    logged.join().unwrap();
    assert_eq!(rec.calls, 0);
    unsafe { blog(LOG_WARNING, c"from this thread".as_ptr()) };
    assert_eq!(rec.calls, 1);
    assert_eq!(rec.level, LOG_WARNING);
    assert_eq!(rec.text(), "from this thread");
}

/// This module's exact test selector for `process_isolated`: these tests
/// replace the process-wide log handler.
fn selector(test: &str) -> String {
    let module = module_path!().split_once("::").unwrap().1;
    format!("{module}::{test}")
}

/// Arms the recorder on `rec`, restores the old handler, and returns the
/// handler and param a `blog` in flight on another thread would still hold.
fn armed_then_restored(rec: &mut Rec) -> (usize, usize) {
    let _saved = Saved::capture();
    arm(rec);
    let mut handler = ptr::null_mut();
    let mut param = ptr::null_mut();
    // SAFETY: the out-pointers are live locals.
    unsafe { base_get_log_handler(&mut handler, &mut param) };
    (handler as usize, param as usize)
}

/// Delivers one line the way `blog` does once it has dropped the mutex.
fn deliver_late((handler, param): (usize, usize)) {
    // SAFETY: `handler` is `base_test_record` and `param` is a `Rec` that
    // outlives the calling thread.
    unsafe {
        base_log_literal(
            handler as *mut c_void,
            param as *mut c_void,
            LOG_WARNING,
            c"late line".as_ptr(),
        )
    };
}

#[test]
fn late_line_from_another_thread_is_ignored_test() {
    if !process_isolated(&selector("late_line_from_another_thread_is_ignored_test")) {
        return;
    }
    // `blog` reads the handler and its param under the base mutex, drops
    // the mutex, then calls the handler. A thread that read them while the
    // recorder was armed can call it after the restore. That line must not
    // land in the record either: by then it is usually a dead stack slot.
    let _gate = lock();
    let mut rec = Rec::new();
    let stale = armed_then_restored(&mut rec);
    std::thread::spawn(move || deliver_late(stale))
        .join()
        .unwrap();
    assert_eq!(rec.calls, 0);
}

#[test]
fn late_line_while_another_thread_arms_is_ignored_test() {
    if !process_isolated(&selector(
        "late_line_while_another_thread_arms_is_ignored_test",
    )) {
        return;
    }
    // The next test can bind its own thread while a late line is still on
    // its way. The owner changes under the late call, and the line still
    // must not land (a data race on the owner shows under ThreadSanitizer).
    let _gate = lock();
    let mut rec = Rec::new();
    let stale = armed_then_restored(&mut rec);
    let late = std::thread::spawn(move || {
        for _ in 0..2_000 {
            deliver_late(stale);
        }
    });
    for _ in 0..50 {
        // SAFETY: binds the recorder's owner only; the handler stays the
        // restored one.
        std::thread::spawn(|| unsafe { base_test_record_only_this_thread() })
            .join()
            .unwrap();
    }
    late.join().unwrap();
    assert_eq!(rec.calls, 0);
}

proptest! {
    #[test]
    fn random_void_name_matches_oracle(name in "[A-Za-z][A-Za-z0-9]{0,12}") {
        let _gate = lock();
        let decl = format!("void {name}()");
        let decl = std::ffi::CString::new(decl).unwrap();
        let call_name = std::ffi::CString::new(name.as_str()).unwrap();
        let missing = std::ffi::CString::new(format!("{name}x")).unwrap();

        let mut c_rec = Rec::new();
        let mut rust_rec = Rec::new();
        let _saved = Saved::capture();

        let c_handler = unsafe { oracle::oracle_proc_handler_create() };
        let rust_handler = unsafe { proc_handler_create() };
        let mut c_seen = Seen { hits: 0, data: 0, params: 0, n: 0 };
        let mut rust_seen = Seen { hits: 0, data: 0, params: 0, n: 0 };

        arm(&mut c_rec);
        let c_result = unsafe {
            oracle::oracle_proc_handler_add(
                c_handler,
                decl.as_ptr(),
                Some(count_call),
                (&mut c_seen as *mut Seen).cast(),
            );
            let hit = oracle::oracle_proc_handler_call(c_handler, call_name.as_ptr(), ptr::null_mut());
            let miss = oracle::oracle_proc_handler_call(c_handler, missing.as_ptr(), ptr::null_mut());
            oracle::oracle_proc_handler_add(
                c_handler,
                decl.as_ptr(),
                Some(bump_seen),
                (&mut c_seen as *mut Seen).cast(),
            );
            let again = oracle::oracle_proc_handler_call(c_handler, call_name.as_ptr(), ptr::null_mut());
            oracle::oracle_proc_handler_destroy(c_handler);
            (hit, miss, again, c_seen.hits, c_rec.calls, c_rec.level, c_rec.text())
        };

        arm(&mut rust_rec);
        unsafe {
            proc_handler_add(
                rust_handler,
                decl.as_ptr(),
                Some(count_call),
                (&mut rust_seen as *mut Seen).cast(),
            );
            let hit = proc_handler_call(rust_handler, call_name.as_ptr(), ptr::null_mut());
            let miss = proc_handler_call(rust_handler, missing.as_ptr(), ptr::null_mut());
            proc_handler_add(
                rust_handler,
                decl.as_ptr(),
                Some(bump_seen),
                (&mut rust_seen as *mut Seen).cast(),
            );
            let again = proc_handler_call(rust_handler, call_name.as_ptr(), ptr::null_mut());
            proc_handler_destroy(rust_handler);
            prop_assert_eq!(c_result.0, hit);
            prop_assert_eq!(c_result.1, miss);
            prop_assert_eq!(c_result.2, again);
            prop_assert!(!miss);
            prop_assert_eq!(c_result.3, rust_seen.hits);
            prop_assert_eq!(c_result.4, rust_rec.calls);
            prop_assert_eq!(c_result.5, rust_rec.level);
            prop_assert_eq!(c_result.6, rust_rec.text());
        }
    }
}
