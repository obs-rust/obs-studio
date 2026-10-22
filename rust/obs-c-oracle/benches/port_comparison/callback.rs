//! #1437: `libobs/callback` (decl, calldata, proc, signal) shims against the
//! C oracles. The fixtures are the shapes libobs uses: a signal declaration,
//! a calldata on a fixed stack (as `obs_source` signals use) and on the
//! heap, a proc call, and a signal to a few callbacks, with and without a
//! global callback.

use std::ffi::{CStr, CString, c_char, c_void};
use std::ptr;

use criterion::Criterion;
use obs_c_oracle::{calldata as c_cd, decl as c_decl, proc as c_proc, signal as c_sig};
use obs_callback::ffi::calldata::{
    calldata, calldata_get_data, calldata_get_string, calldata_set_data,
};
use obs_callback::ffi::decl::{decl_info, parse_decl_string};
use obs_callback::ffi::{proc as rs_proc, signal as rs_sig};
use obs_util::ffi::bmem::bfree;
use obs_util::ffi::darray::darray;

use crate::pair;

const DECL: &CStr = c"void source_create(ptr source, string name, int flags, out bool result)";
const SIGNALS: usize = 16;
const CALLBACKS: usize = 4;

type SetData = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_void, usize);
type GetData = unsafe extern "C" fn(*const c_void, *const c_char, *mut c_void, usize) -> bool;
type GetString = unsafe extern "C" fn(*const c_void, *const c_char, *mut *const c_char) -> bool;
type Parse = unsafe extern "C" fn(*mut c_void, *const c_char) -> bool;
type Callback = unsafe extern "C" fn(*mut c_void, *mut c_void);
type GlobalCallback = unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_void);

// The Rust shims take typed pointers; the oracles take `void *`.

unsafe extern "C" fn rs_set(
    data: *mut c_void,
    name: *const c_char,
    input: *const c_void,
    size: usize,
) {
    // SAFETY: forwarded.
    unsafe { calldata_set_data(data.cast(), name, input, size) }
}
unsafe extern "C" fn rs_get(
    data: *const c_void,
    name: *const c_char,
    out: *mut c_void,
    size: usize,
) -> bool {
    // SAFETY: forwarded.
    unsafe { calldata_get_data(data.cast(), name, out, size) }
}
unsafe extern "C" fn rs_get_string(
    data: *const c_void,
    name: *const c_char,
    out: *mut *const c_char,
) -> bool {
    // SAFETY: forwarded.
    unsafe { calldata_get_string(data.cast(), name, out) }
}
unsafe extern "C" fn rs_parse(decl: *mut c_void, text: *const c_char) -> bool {
    // SAFETY: forwarded.
    unsafe { parse_decl_string(decl.cast(), text) }
}

/// `calldata_init_fixed` on a zeroed buffer, or `calldata_init` (heap).
struct Stack {
    data: calldata,
    _buf: Option<Box<[u8; 256]>>,
}

impl Stack {
    fn fixed() -> Self {
        let mut buf = Box::new([0u8; 256]);
        Self {
            data: calldata {
                stack: buf.as_mut_ptr(),
                // calldata_clear: the stack starts with a zeroed size_t
                size: size_of::<usize>(),
                capacity: buf.len(),
                fixed: true,
            },
            _buf: Some(buf),
        }
    }

    fn heap() -> Self {
        Self {
            data: calldata {
                stack: ptr::null_mut(),
                size: 0,
                capacity: 0,
                fixed: false,
            },
            _buf: None,
        }
    }

    fn ptr(&mut self) -> *mut c_void {
        (&raw mut self.data).cast()
    }
}

impl Drop for Stack {
    fn drop(&mut self) {
        if !self.data.fixed && !self.data.stack.is_null() {
            // SAFETY: a heap stack is `bmalloc` memory (`calldata_free`).
            unsafe { bfree(self.data.stack.cast()) };
        }
    }
}

/// The typed `calldata_set_*` / `calldata_get_*` header inlines, over
/// `set` / `get`: the parameters of a `source_create` call, then each one
/// read back.
fn set_get(
    stack: &mut Stack,
    set: SetData,
    get: GetData,
    get_string: GetString,
) -> (usize, i64, f64, usize) {
    let source = 0x1234_usize as *mut c_void;
    let flags = 7_i64;
    let volume = 0.5_f64;
    let name = c"Capture Card";
    let data = stack.ptr();
    // SAFETY: `data` is a valid calldata; each input is valid for its size.
    unsafe {
        set(
            data,
            c"source".as_ptr(),
            (&raw const source).cast(),
            size_of::<*mut c_void>(),
        );
        set(
            data,
            c"name".as_ptr(),
            name.as_ptr().cast(),
            name.to_bytes_with_nul().len(),
        );
        set(
            data,
            c"flags".as_ptr(),
            (&raw const flags).cast(),
            size_of::<i64>(),
        );
        set(
            data,
            c"volume".as_ptr(),
            (&raw const volume).cast(),
            size_of::<f64>(),
        );

        let mut out_source: *mut c_void = ptr::null_mut();
        let mut out_flags = 0_i64;
        let mut out_volume = 0_f64;
        let mut out_name: *const c_char = ptr::null();
        get(
            data,
            c"source".as_ptr(),
            (&raw mut out_source).cast(),
            size_of::<*mut c_void>(),
        );
        get(
            data,
            c"flags".as_ptr(),
            (&raw mut out_flags).cast(),
            size_of::<i64>(),
        );
        get(
            data,
            c"volume".as_ptr(),
            (&raw mut out_volume).cast(),
            size_of::<f64>(),
        );
        get_string(data, c"name".as_ptr(), &raw mut out_name);
        (
            out_source as usize,
            out_flags,
            out_volume,
            CStr::from_ptr(out_name).to_bytes().len(),
        )
    }
}

fn calldata_benchmarks(criterion: &mut Criterion) {
    pair(
        criterion,
        "calldata/fixed/set_get_4",
        || set_get(&mut Stack::fixed(), rs_set, rs_get, rs_get_string),
        || {
            set_get(
                &mut Stack::fixed(),
                c_cd::oracle_calldata_set_data,
                c_cd::oracle_calldata_get_data,
                c_cd::oracle_calldata_get_string,
            )
        },
    );
    pair(
        criterion,
        "calldata/heap/set_get_4",
        || set_get(&mut Stack::heap(), rs_set, rs_get, rs_get_string),
        || {
            set_get(
                &mut Stack::heap(),
                c_cd::oracle_calldata_set_data,
                c_cd::oracle_calldata_get_data,
                c_cd::oracle_calldata_get_string,
            )
        },
    );
}

/// `parse_decl_string` on `DECL`, then `decl_info_free`.
fn parse(parse: Parse) -> (bool, usize) {
    let mut info = decl_info {
        name: ptr::null_mut(),
        decl_string: ptr::null(),
        params: darray {
            array: ptr::null_mut(),
            num: 0,
            capacity: 0,
        },
    };
    // SAFETY: a zeroed decl_info and a NUL-terminated declaration.
    let ok = unsafe { parse((&raw mut info).cast(), DECL.as_ptr()) };
    let params = info.params.num;
    // SAFETY: `decl_info_free`: the names and the array are `bmalloc` or null.
    unsafe {
        let array = info
            .params
            .array
            .cast::<obs_callback::ffi::decl::decl_param>();
        for i in 0..params {
            bfree((*array.add(i)).name.cast());
        }
        bfree(info.params.array);
        bfree(info.name.cast());
    }
    (ok, params)
}

fn decl_benchmarks(criterion: &mut Criterion) {
    pair(
        criterion,
        "decl/parse/4",
        || parse(rs_parse),
        || parse(c_decl::oracle_parse_decl_string),
    );
}

unsafe extern "C" fn count(data: *mut c_void, _params: *mut c_void) {
    // SAFETY: `data` is the `u64` counter of the fixture.
    unsafe { *data.cast::<u64>() += 1 };
}

unsafe extern "C" fn count_global(data: *mut c_void, _signal: *const c_char, _params: *mut c_void) {
    // SAFETY: `data` is the `u64` counter of the fixture.
    unsafe { *data.cast::<u64>() += 1 };
}

struct ProcSide {
    create: unsafe extern "C" fn() -> *mut c_void,
    destroy: unsafe extern "C" fn(*mut c_void),
    add: unsafe extern "C" fn(*mut c_void, *const c_char, Option<Callback>, *mut c_void),
    call: unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_void) -> bool,
}

unsafe extern "C" fn rs_proc_create() -> *mut c_void {
    // SAFETY: no preconditions.
    unsafe { rs_proc::proc_handler_create().cast() }
}
unsafe extern "C" fn rs_proc_destroy(h: *mut c_void) {
    // SAFETY: forwarded.
    unsafe { rs_proc::proc_handler_destroy(h.cast()) }
}
unsafe extern "C" fn rs_proc_add(
    h: *mut c_void,
    decl: *const c_char,
    proc: Option<Callback>,
    data: *mut c_void,
) {
    // SAFETY: forwarded.
    unsafe { rs_proc::proc_handler_add(h.cast(), decl, proc, data) }
}
unsafe extern "C" fn rs_proc_call(
    h: *mut c_void,
    name: *const c_char,
    params: *mut c_void,
) -> bool {
    // SAFETY: forwarded.
    unsafe { rs_proc::proc_handler_call(h.cast(), name, params) }
}

/// A proc handler with `SIGNALS` procs; each call runs the last one added.
struct Procs {
    side: ProcSide,
    handler: *mut c_void,
    counter: Box<u64>,
    stack: Stack,
}

impl Procs {
    fn new(side: ProcSide) -> Self {
        let mut counter = Box::new(0_u64);
        // SAFETY: a fresh handler; the declarations are NUL-terminated and
        // the counter outlives the handler.
        let handler = unsafe { (side.create)() };
        for i in 0..SIGNALS {
            let decl = CString::new(format!("void p{i:02}(ptr source, int n)")).unwrap();
            unsafe {
                (side.add)(
                    handler,
                    decl.as_ptr(),
                    Some(count),
                    (&raw mut *counter).cast(),
                )
            };
        }
        let mut stack = Stack::fixed();
        let n = 3_i64;
        // SAFETY: a valid fixed calldata.
        unsafe {
            calldata_set_data(
                &raw mut stack.data,
                c"n".as_ptr(),
                (&raw const n).cast(),
                size_of::<i64>(),
            )
        };
        Self {
            side,
            handler,
            counter,
            stack,
        }
    }

    fn call(&mut self) -> u64 {
        let params = self.stack.ptr();
        // SAFETY: a live handler and calldata.
        let ok = unsafe { (self.side.call)(self.handler, c"p15".as_ptr(), params) };
        assert!(ok);
        *self.counter
    }
}

impl Drop for Procs {
    fn drop(&mut self) {
        // SAFETY: the handler is live and destroyed once.
        unsafe { (self.side.destroy)(self.handler) };
    }
}

fn proc_benchmarks(criterion: &mut Criterion) {
    let mut rust = Procs::new(ProcSide {
        create: rs_proc_create,
        destroy: rs_proc_destroy,
        add: rs_proc_add,
        call: rs_proc_call,
    });
    let mut c = Procs::new(ProcSide {
        create: c_proc::oracle_proc_handler_create,
        destroy: c_proc::oracle_proc_handler_destroy,
        add: c_proc::oracle_proc_handler_add,
        call: c_proc::oracle_proc_handler_call,
    });
    // The counters advance together, so each pair returns the same count.
    pair(criterion, "proc/call/16", || rust.call(), || c.call());
}

struct SignalSide {
    create: unsafe extern "C" fn() -> *mut c_void,
    destroy: unsafe extern "C" fn(*mut c_void),
    add: unsafe extern "C" fn(*mut c_void, *const c_char) -> bool,
    connect: unsafe extern "C" fn(*mut c_void, *const c_char, Option<Callback>, *mut c_void),
    connect_global: unsafe extern "C" fn(*mut c_void, Option<GlobalCallback>, *mut c_void),
    signal: unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_void),
}

unsafe extern "C" fn rs_sig_create() -> *mut c_void {
    rs_sig::signal_handler_create().cast()
}
unsafe extern "C" fn rs_sig_destroy(h: *mut c_void) {
    // SAFETY: forwarded.
    unsafe { rs_sig::signal_handler_destroy(h.cast()) }
}
unsafe extern "C" fn rs_sig_add(h: *mut c_void, decl: *const c_char) -> bool {
    // SAFETY: forwarded.
    unsafe { rs_sig::signal_handler_add(h.cast(), decl) }
}
unsafe extern "C" fn rs_sig_connect(
    h: *mut c_void,
    signal: *const c_char,
    callback: Option<Callback>,
    data: *mut c_void,
) {
    // SAFETY: forwarded; the callback types differ only in a pointer type.
    unsafe {
        rs_sig::signal_handler_connect(
            h.cast(),
            signal,
            std::mem::transmute::<Option<Callback>, rs_sig::signal_callback_t>(callback),
            data,
        )
    }
}
unsafe extern "C" fn rs_sig_connect_global(
    h: *mut c_void,
    callback: Option<GlobalCallback>,
    data: *mut c_void,
) {
    // SAFETY: forwarded; the callback types differ only in a pointer type.
    unsafe {
        rs_sig::signal_handler_connect_global(
            h.cast(),
            std::mem::transmute::<Option<GlobalCallback>, rs_sig::global_signal_callback_t>(
                callback,
            ),
            data,
        )
    }
}
unsafe extern "C" fn rs_sig_signal(h: *mut c_void, signal: *const c_char, params: *mut c_void) {
    // SAFETY: forwarded.
    unsafe { rs_sig::signal_handler_signal(h.cast(), signal, params.cast()) }
}

/// A signal handler with `SIGNALS` signals and `callbacks` callbacks on
/// the last one (plus one global callback when `global`). Each callback
/// has its own counter: both sides match callbacks by callback and data,
/// so the same pair connects once.
struct Signals {
    side: SignalSide,
    handler: *mut c_void,
    counters: Box<[u64; CALLBACKS + 1]>,
    stack: Stack,
}

impl Signals {
    fn new(side: SignalSide, callbacks: usize, global: bool) -> Self {
        let mut counters = Box::new([0_u64; CALLBACKS + 1]);
        let data: *mut u64 = counters.as_mut_ptr();
        // SAFETY: a fresh handler; the declarations are NUL-terminated and
        // the counters outlive the handler.
        let handler = unsafe { (side.create)() };
        unsafe {
            for i in 0..SIGNALS {
                let decl = CString::new(format!("void s{i:02}(ptr source)")).unwrap();
                assert!((side.add)(handler, decl.as_ptr()));
            }
            for i in 0..callbacks {
                (side.connect)(handler, c"s15".as_ptr(), Some(count), data.add(i).cast());
            }
            if global {
                (side.connect_global)(handler, Some(count_global), data.add(CALLBACKS).cast());
            }
        }
        let mut stack = Stack::fixed();
        let source = 0x1234_usize;
        // SAFETY: a valid fixed calldata.
        unsafe {
            calldata_set_data(
                &raw mut stack.data,
                c"source".as_ptr(),
                (&raw const source).cast(),
                size_of::<usize>(),
            )
        };
        Self {
            side,
            handler,
            counters,
            stack,
        }
    }

    fn signal(&mut self) -> u64 {
        let params = self.stack.ptr();
        // SAFETY: a live handler and calldata.
        unsafe { (self.side.signal)(self.handler, c"s15".as_ptr(), params) };
        self.counters.iter().sum()
    }
}

impl Drop for Signals {
    fn drop(&mut self) {
        // SAFETY: the handler is live and destroyed once.
        unsafe { (self.side.destroy)(self.handler) };
    }
}

fn rust_signals() -> SignalSide {
    SignalSide {
        create: rs_sig_create,
        destroy: rs_sig_destroy,
        add: rs_sig_add,
        connect: rs_sig_connect,
        connect_global: rs_sig_connect_global,
        signal: rs_sig_signal,
    }
}

fn c_signals() -> SignalSide {
    SignalSide {
        create: c_sig::oracle_signal_handler_create,
        destroy: c_sig::oracle_signal_handler_destroy,
        add: c_sig::oracle_signal_handler_add,
        connect: c_sig::oracle_signal_handler_connect,
        connect_global: c_sig::oracle_signal_handler_connect_global,
        signal: c_sig::oracle_signal_handler_signal,
    }
}

fn signal_benchmarks(criterion: &mut Criterion) {
    for (name, callbacks, global) in [
        ("signal/emit/0", 0, false),
        ("signal/emit/4", CALLBACKS, false),
        ("signal/emit_global/4", CALLBACKS, true),
    ] {
        let mut rust = Signals::new(rust_signals(), callbacks, global);
        let mut c = Signals::new(c_signals(), callbacks, global);
        pair(criterion, name, || rust.signal(), || c.signal());
    }
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    decl_benchmarks(criterion);
    calldata_benchmarks(criterion);
    proc_benchmarks(criterion);
    signal_benchmarks(criterion);
}
