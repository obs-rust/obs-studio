//! Tier 3: the calldata shims match `libobs/callback/calldata.c.h`.
//!
//! Compared state is the struct fields and the used stack bytes. The gap
//! past `size` is uninitialized in C. Null `out` pointers and a null stack
//! with a non-zero size are the shim's null guards; this file does not
//! generate them.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use std::ffi::{CStr, CString, c_char, c_void};
use std::mem::size_of;
use std::ptr;

use obs_c_oracle::calldata as oracle;
use obs_callback::ffi::calldata::{
    calldata, calldata_get_data, calldata_get_string, calldata_set_data,
};
use obs_util as _;
use proptest::prelude::*;

unsafe extern "C" {
    fn bfree(ptr: *mut c_void);
}

fn fresh() -> calldata {
    calldata {
        stack: ptr::null_mut(),
        size: 0,
        capacity: 0,
        fixed: false,
    }
}

struct Heap(calldata);

impl Drop for Heap {
    fn drop(&mut self) {
        if !self.0.fixed && !self.0.stack.is_null() {
            unsafe { bfree(self.0.stack.cast()) };
        }
    }
}

fn clear(cd: &mut calldata) {
    if !cd.stack.is_null() {
        cd.size = size_of::<usize>();
        unsafe { ptr::write_bytes(cd.stack, 0, size_of::<usize>()) };
    }
}

fn init_fixed(cd: &mut calldata, buf: &mut [u8]) {
    cd.stack = buf.as_mut_ptr();
    cd.capacity = buf.len();
    cd.fixed = true;
    cd.size = 0;
    clear(cd);
}

fn used(cd: &calldata) -> Vec<u8> {
    if cd.stack.is_null() || cd.size == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(cd.stack, cd.size).to_vec() }
    }
}

fn assert_same(left: &calldata, right: &calldata) {
    assert_eq!(left.size, right.size, "size");
    assert_eq!(left.capacity, right.capacity, "capacity");
    assert_eq!(left.fixed, right.fixed, "fixed");
    assert_eq!(left.stack.is_null(), right.stack.is_null(), "stack null");
    assert_eq!(used(left), used(right), "stack bytes");
}

fn set_both(c_side: &mut calldata, rust_side: &mut calldata, name: &str, bytes: &[u8]) {
    let name = CString::new(name).unwrap();
    let input = bytes.as_ptr().cast::<c_void>();
    unsafe {
        oracle::oracle_calldata_set_data(
            c_side as *mut calldata as *mut c_void,
            name.as_ptr(),
            input,
            bytes.len(),
        );
        calldata_set_data(rust_side, name.as_ptr(), input, bytes.len());
    }
}

fn get_both(
    c_side: &calldata,
    rust_side: &calldata,
    name: &str,
    size: usize,
) -> (bool, bool, Vec<u8>, Vec<u8>) {
    let name = CString::new(name).unwrap();
    let mut c_out = vec![0xA5u8; size];
    let mut r_out = vec![0xA5u8; size];
    let c_ok = unsafe {
        oracle::oracle_calldata_get_data(
            c_side as *const calldata as *const c_void,
            name.as_ptr(),
            c_out.as_mut_ptr().cast(),
            size,
        )
    };
    let r_ok =
        unsafe { calldata_get_data(rust_side, name.as_ptr(), r_out.as_mut_ptr().cast(), size) };
    (c_ok, r_ok, c_out, r_out)
}

fn assert_gets(c_side: &calldata, rust_side: &calldata, name: &str, size: usize) {
    let (c_ok, r_ok, c_out, r_out) = get_both(c_side, rust_side, name, size);
    assert_eq!(c_ok, r_ok, "get {name} size {size}");
    assert_eq!(c_out, r_out, "get bytes {name}");
    let (c_ok, r_ok, c_str, r_str) = string_both(c_side, rust_side, name);
    assert_eq!(c_ok, r_ok, "get_string {name}");
    assert_eq!(c_str, r_str, "string bytes {name}");
}

fn string_both(
    c_side: &calldata,
    rust_side: &calldata,
    name: &str,
) -> (bool, bool, Option<Vec<u8>>, Option<Vec<u8>>) {
    let name = CString::new(name).unwrap();
    let mut c_ptr: *const c_char = ptr::without_provenance(1);
    let mut r_ptr: *const c_char = ptr::without_provenance(1);
    let c_ok = unsafe {
        oracle::oracle_calldata_get_string(
            c_side as *const calldata as *const c_void,
            name.as_ptr(),
            &mut c_ptr,
        )
    };
    let r_ok = unsafe { calldata_get_string(rust_side, name.as_ptr(), &mut r_ptr) };
    let c_bytes = if c_ok { c_string_bytes(c_ptr) } else { None };
    let r_bytes = if r_ok { c_string_bytes(r_ptr) } else { None };
    (c_ok, r_ok, c_bytes, r_bytes)
}

/// `None` for a null pointer. Otherwise the bytes through the first NUL,
/// inclusive, capped so a missing terminator cannot run off the stack.
fn c_string_bytes(ptr: *const c_char) -> Option<Vec<u8>> {
    if ptr.is_null() {
        return None;
    }
    let mut out = Vec::new();
    for i in 0..64 {
        let byte = unsafe { *ptr.add(i) as u8 };
        out.push(byte);
        if byte == 0 {
            break;
        }
    }
    Some(out)
}

#[test]
fn first_int_matches_oracle() {
    let mut c_side = Heap(fresh());
    let mut rust_side = Heap(fresh());
    let value = 42i64;
    set_both(&mut c_side.0, &mut rust_side.0, "a", &value.to_ne_bytes());
    assert_same(&c_side.0, &rust_side.0);
    assert_eq!(c_side.0.capacity, 128);
    assert_gets(&c_side.0, &rust_side.0, "a", size_of::<i64>());
    assert_gets(&c_side.0, &rust_side.0, "missing", size_of::<i64>());
}

#[test]
fn resize_and_string_match_oracle() {
    let mut c_side = Heap(fresh());
    let mut rust_side = Heap(fresh());
    set_both(&mut c_side.0, &mut rust_side.0, "a", &11i64.to_ne_bytes());
    set_both(&mut c_side.0, &mut rust_side.0, "b", &22i64.to_ne_bytes());
    assert_same(&c_side.0, &rust_side.0);

    let grown = [0x3Cu8; 32];
    set_both(&mut c_side.0, &mut rust_side.0, "a", &grown);
    assert_same(&c_side.0, &rust_side.0);
    assert_gets(&c_side.0, &rust_side.0, "a", grown.len());
    assert_gets(&c_side.0, &rust_side.0, "b", size_of::<i64>());

    set_both(&mut c_side.0, &mut rust_side.0, "a", &33i64.to_ne_bytes());
    set_both(&mut c_side.0, &mut rust_side.0, "a", &[]);
    assert_same(&c_side.0, &rust_side.0);

    let hello = b"hello\0";
    set_both(&mut c_side.0, &mut rust_side.0, "hello", hello);
    set_both(&mut c_side.0, &mut rust_side.0, "none", &[]);
    set_both(&mut c_side.0, &mut rust_side.0, "blank", b"\0");
    assert_same(&c_side.0, &rust_side.0);
    assert_gets(&c_side.0, &rust_side.0, "hello", hello.len());
    assert_gets(&c_side.0, &rust_side.0, "none", 0);
    assert_gets(&c_side.0, &rust_side.0, "blank", 1);
}

#[test]
fn exact_fit_and_past_double_match_oracle() {
    let mut c_side = Heap(fresh());
    let mut rust_side = Heap(fresh());
    set_both(&mut c_side.0, &mut rust_side.0, "a", b"Z");
    let size = c_side.0.size;
    let extra = 128 - size;
    let overhead = 2 + size_of::<usize>() * 2;
    let blob = vec![0x11u8; extra - overhead];
    set_both(&mut c_side.0, &mut rust_side.0, "b", &blob);
    assert_same(&c_side.0, &rust_side.0);
    assert_eq!(c_side.0.capacity, 128);

    let mut c_side = Heap(fresh());
    let mut rust_side = Heap(fresh());
    set_both(&mut c_side.0, &mut rust_side.0, "a", &5i64.to_ne_bytes());
    let overhead = 2 + size_of::<usize>() * 2;
    let data_size = 300 - c_side.0.size - overhead;
    set_both(
        &mut c_side.0,
        &mut rust_side.0,
        "b",
        &vec![0x22u8; data_size],
    );
    assert_same(&c_side.0, &rust_side.0);
    assert_eq!(c_side.0.size, 300);
    assert_eq!(c_side.0.capacity, 300);
}

#[test]
fn fixed_stack_matches_oracle() {
    let needed = size_of::<usize>() * 3 + 2 + size_of::<i64>();
    let mut c_buf = vec![0xABu8; needed + 1];
    let mut r_buf = c_buf.clone();
    let mut c_side = Heap(fresh());
    let mut rust_side = Heap(fresh());
    init_fixed(&mut c_side.0, &mut c_buf);
    init_fixed(&mut rust_side.0, &mut r_buf);
    set_both(&mut c_side.0, &mut rust_side.0, "a", &42i64.to_ne_bytes());
    assert_same(&c_side.0, &rust_side.0);
    let bigger = vec![7u8; size_of::<i64>() + 2];
    set_both(&mut c_side.0, &mut rust_side.0, "a", &bigger);
    assert_same(&c_side.0, &rust_side.0);
    assert_gets(&c_side.0, &rust_side.0, "a", size_of::<i64>());

    let mut c_exact = vec![0u8; needed];
    let mut r_exact = c_exact.clone();
    let mut c_side = Heap(fresh());
    let mut rust_side = Heap(fresh());
    init_fixed(&mut c_side.0, &mut c_exact);
    init_fixed(&mut rust_side.0, &mut r_exact);
    set_both(&mut c_side.0, &mut rust_side.0, "a", &5i64.to_ne_bytes());
    assert_same(&c_side.0, &rust_side.0);
    assert_eq!(c_side.0.size, needed);
}

#[derive(Clone, Debug)]
enum Op {
    Set { name: u8, len: u8 },
    Clear,
}

fn apply(op: &Op, c_side: &mut calldata, rust_side: &mut calldata) {
    match *op {
        Op::Clear => {
            clear(c_side);
            clear(rust_side);
        }
        Op::Set { name, len } => {
            let name = [b'a' + (name % 4)];
            let name = std::str::from_utf8(&name).unwrap();
            let byte = len;
            let payload = vec![byte; len as usize];
            set_both(c_side, rust_side, name, &payload);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(64)))]

    #[test]
    fn random_sets_match_oracle(ops in prop::collection::vec(
        prop_oneof![
            (0u8..4, 0u8..48).prop_map(|(name, len)| Op::Set { name, len }),
            Just(Op::Clear),
        ],
        1..12
    )) {
        let mut c_side = Heap(fresh());
        let mut rust_side = Heap(fresh());
        for op in &ops {
            apply(op, &mut c_side.0, &mut rust_side.0);
            assert_same(&c_side.0, &rust_side.0);
        }
    }

    #[test]
    fn random_fixed_sets_match_oracle(
        cap in 16usize..160,
        ops in prop::collection::vec(
            (0u8..3, 0u8..24).prop_map(|(name, len)| Op::Set { name, len }),
            1..8
        )
    ) {
        let mut c_buf = vec![0x5Au8; cap];
        let mut r_buf = c_buf.clone();
        let mut c_side = Heap(fresh());
        let mut rust_side = Heap(fresh());
        init_fixed(&mut c_side.0, &mut c_buf);
        init_fixed(&mut rust_side.0, &mut r_buf);
        for op in &ops {
            apply(op, &mut c_side.0, &mut rust_side.0);
            assert_same(&c_side.0, &rust_side.0);
        }
    }
}

/// `calldata_set_data` with an input inside the same stack, in the cases
/// C defines (the input does not move): a new parameter from an earlier
/// one's data, and a later parameter replaced, larger, with an earlier
/// one's data. The shim edits the stack in place (#1749) only when the
/// input is outside it.
#[test]
fn input_inside_the_stack_matches_oracle() {
    fn run(
        cd: &mut calldata,
        set: &dyn Fn(*mut calldata, &CStr, *const c_void, usize),
        get_string: &dyn Fn(*const calldata, &CStr) -> *const c_char,
    ) {
        set(cd, c"a", c"hello".as_ptr().cast(), 6);
        set(cd, c"b", 7_i64.to_ne_bytes().as_ptr().cast(), 8);
        set(cd, c"c", c"xy".as_ptr().cast(), 3);
        let a = get_string(cd, c"a");
        set(cd, c"d", a.cast(), 6);
        let a = get_string(cd, c"a");
        set(cd, c"c", a.cast(), 6);
    }
    let rust_set = |cd: *mut calldata, name: &CStr, input: *const c_void, size: usize| unsafe {
        calldata_set_data(cd, name.as_ptr(), input, size)
    };
    let c_set = |cd: *mut calldata, name: &CStr, input: *const c_void, size: usize| unsafe {
        oracle::oracle_calldata_set_data(cd.cast(), name.as_ptr(), input, size)
    };
    let rust_get = |cd: *const calldata, name: &CStr| {
        let mut out: *const c_char = ptr::null();
        assert!(unsafe { calldata_get_string(cd, name.as_ptr(), &mut out) });
        out
    };
    let c_get = |cd: *const calldata, name: &CStr| {
        let mut out: *const c_char = ptr::null();
        assert!(unsafe { oracle::oracle_calldata_get_string(cd.cast(), name.as_ptr(), &mut out) });
        out
    };

    let (mut c_buf, mut r_buf) = ([0u8; 256], [0u8; 256]);
    let (mut c_side, mut rust_side) = (fresh(), fresh());
    init_fixed(&mut c_side, &mut c_buf);
    init_fixed(&mut rust_side, &mut r_buf);
    run(&mut c_side, &c_set, &c_get);
    run(&mut rust_side, &rust_set, &rust_get);
    assert_same(&c_side, &rust_side);

    let (mut c_side, mut rust_side) = (Heap(fresh()), Heap(fresh()));
    run(&mut c_side.0, &c_set, &c_get);
    run(&mut rust_side.0, &rust_set, &rust_get);
    assert_same(&c_side.0, &rust_side.0);
    for name in ["a", "b", "c", "d"] {
        assert_gets(&c_side.0, &rust_side.0, name, 6);
    }
}
