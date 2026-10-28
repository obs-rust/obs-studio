//! Tier 3: `obs_core::nix_system_info` against the original C.
//!
//! `obs-c-oracle` compiles the unchanged `libobs/obs-nix-system-info.c.h`
//! with its eleven functions renamed `oracle_*`. Every test runs the Rust
//! shim, the C and the safe core on the same input and compares the results
//! exactly: the C's `bool` as the byte it returns, a returned pointer as its
//! offset into the line (or NULL), `*len` as written (or untouched).
//!
//! - every line of up to three pieces of the cpuinfo and dmesg.boot
//!   alphabet, every byte alone and after each key, and this host's
//!   /proc/cpuinfo lines when it has one;
//! - `atoi()` over number texts: every value from -70,000 to 70,000 and the
//!   ends of `int`, with leading white space, signs and trailing text;
//! - the memory sizes over the edges of every field;
//! - both callers' loops, with the shims and with the C, over the C test's
//!   texts and generated files: the same blocks and speeds;
//! - proptests (20,000 cases each) over all of it.
//!
//! `atoi()` past the bounds of `int` is undefined in C. glibc and macOS
//! return `(int)strtol()`, which the core matches; MSVC saturates at `int`.
//! So only Linux compares number texts out of range, in a line, a value or
//! a caller's file; every host compares the rest. No intentional
//! differences.

use core::ffi::{c_char, c_int};
use core::ops::Range;
use std::ffi::CString;
use std::ptr;

use obs_c_oracle::nix_system_info as oracle;
use obs_core::ffi::nix_system_info as shim;
use obs_core::nix_system_info as core_rules;
use proptest::prelude::*;

/// The pieces the lines are made of.
const PIECES: &[&[u8]] = &[
    b"model name",
    b"model nam",
    b"physical id",
    b"physical i",
    b"cpu MHz",
    b"cpu MH",
    b"CPU: ",
    b"CPU:",
    b":",
    b"\t: ",
    b" ",
    b"\n",
    b"\r",
    b"\x0b",
    b"-",
    b"+",
    b"(",
    b")",
    b"0",
    b"42",
    b"2147483647",
    b"x",
    b"\xc3\xa9",
    b"\xff",
];

/// What one line gives on one side: the four key checks, the value offset,
/// the physical id of that value, the print rule for three id pairs, and
/// the dmesg.boot speed with `*len`.
#[derive(Debug, PartialEq, Eq)]
struct LineResult {
    keys: [u8; 4],
    value: Option<usize>,
    physical_id: Option<c_int>,
    prints: [u8; 3],
    speed: Option<usize>,
    len: usize,
}

/// The offset of `p` in `base`, or `None` for NULL.
fn offset(base: *const c_char, p: *const c_char) -> Option<usize> {
    (!p.is_null()).then(|| p as usize - base as usize)
}

/// A line's results through one side's functions.
macro_rules! line_result {
    ($line:expr, $is_name:path, $is_physical_id:path, $is_speed:path, $is_dmesg_cpu:path,
     $value:path, $physical_id:path, $prints:path, $dmesg_speed:path) => {{
        let line: &CString = $line;
        let p = line.as_ptr();
        // SAFETY: `p` is a live NUL-terminated string, and `len` a writable
        // size_t; the results point into `p` or are NULL.
        unsafe {
            let value = offset(p, $value(p));
            let physical_id = value.map(|v| $physical_id(p.add(v)));
            let mut len = usize::MAX;
            let speed = offset(p, $dmesg_speed(p, &mut len));
            LineResult {
                keys: [
                    u8::from($is_name(p)),
                    u8::from($is_physical_id(p)),
                    u8::from($is_speed(p)),
                    u8::from($is_dmesg_cpu(p)),
                ],
                value,
                physical_id,
                prints: [
                    u8::from($prints(p, 0, -1)),
                    u8::from($prints(p, 3, 3)),
                    u8::from($prints(p, c_int::MIN, c_int::MAX)),
                ],
                speed,
                len,
            }
        }
    }};
}

/// `u8::from` for the oracle's byte, so both sides share the macro.
trait AsByte {
    fn byte(self) -> u8;
}
impl AsByte for u8 {
    fn byte(self) -> u8 {
        self
    }
}
impl AsByte for bool {
    fn byte(self) -> u8 {
        u8::from(self)
    }
}

unsafe fn o_is_name(p: *const c_char) -> u8 {
    // SAFETY: forwarded.
    unsafe { oracle::oracle_obs_nix_system_info_is_name(p) }.byte()
}
unsafe fn o_is_physical_id(p: *const c_char) -> u8 {
    // SAFETY: forwarded.
    unsafe { oracle::oracle_obs_nix_system_info_is_physical_id(p) }.byte()
}
unsafe fn o_is_speed(p: *const c_char) -> u8 {
    // SAFETY: forwarded.
    unsafe { oracle::oracle_obs_nix_system_info_is_speed(p) }.byte()
}
unsafe fn o_is_dmesg_cpu(p: *const c_char) -> u8 {
    // SAFETY: forwarded.
    unsafe { oracle::oracle_obs_nix_system_info_is_dmesg_cpu(p) }.byte()
}
unsafe fn o_prints(p: *const c_char, id: c_int, last: c_int) -> u8 {
    // SAFETY: forwarded.
    unsafe { oracle::oracle_obs_nix_system_info_prints(p, id, last) }.byte()
}
unsafe fn s_is_name(p: *const c_char) -> u8 {
    // SAFETY: forwarded.
    unsafe { shim::obs_nix_system_info_is_name(p) }.byte()
}
unsafe fn s_is_physical_id(p: *const c_char) -> u8 {
    // SAFETY: forwarded.
    unsafe { shim::obs_nix_system_info_is_physical_id(p) }.byte()
}
unsafe fn s_is_speed(p: *const c_char) -> u8 {
    // SAFETY: forwarded.
    unsafe { shim::obs_nix_system_info_is_speed(p) }.byte()
}
unsafe fn s_is_dmesg_cpu(p: *const c_char) -> u8 {
    // SAFETY: forwarded.
    unsafe { shim::obs_nix_system_info_is_dmesg_cpu(p) }.byte()
}
unsafe fn s_prints(p: *const c_char, id: c_int, last: c_int) -> u8 {
    // SAFETY: forwarded.
    unsafe { shim::obs_nix_system_info_prints(p, id, last) }.byte()
}

fn c_result(line: &CString) -> LineResult {
    line_result!(
        line,
        o_is_name,
        o_is_physical_id,
        o_is_speed,
        o_is_dmesg_cpu,
        oracle::oracle_obs_nix_system_info_value,
        oracle::oracle_obs_nix_system_info_physical_id,
        o_prints,
        oracle::oracle_obs_nix_system_info_dmesg_speed
    )
}

fn shim_result(line: &CString) -> LineResult {
    line_result!(
        line,
        s_is_name,
        s_is_physical_id,
        s_is_speed,
        s_is_dmesg_cpu,
        shim::obs_nix_system_info_value,
        shim::obs_nix_system_info_physical_id,
        s_prints,
        shim::obs_nix_system_info_dmesg_speed
    )
}

fn core_result(line: &[u8]) -> LineResult {
    let value = core_rules::value(line);
    let speed: Option<Range<usize>> = core_rules::dmesg_speed(line);
    LineResult {
        keys: [
            u8::from(core_rules::is_name(line)),
            u8::from(core_rules::is_physical_id(line)),
            u8::from(core_rules::is_speed(line)),
            u8::from(core_rules::is_dmesg_cpu(line)),
        ],
        value,
        physical_id: value.map(|v| core_rules::physical_id(&line[v..])),
        prints: [
            u8::from(core_rules::prints(line, 0, -1)),
            u8::from(core_rules::prints(line, 3, 3)),
            u8::from(core_rules::prints(line, c_int::MIN, c_int::MAX)),
        ],
        speed: speed.as_ref().map(|r| r.start),
        len: speed.map_or(usize::MAX, |r| r.len()),
    }
}

/// Whether a value's number text is out of `int` range, where `atoi()` is
/// undefined and only glibc's `(int)strtol()` is compared.
fn atoi_out_of_range(value: &[u8]) -> bool {
    let text: Vec<u8> = value
        .iter()
        .copied()
        .skip_while(|b| matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r'))
        .collect();
    let digits = text
        .strip_prefix(b"-")
        .or_else(|| text.strip_prefix(b"+"))
        .unwrap_or(&text);
    let digits: Vec<u8> = digits
        .iter()
        .copied()
        .take_while(u8::is_ascii_digit)
        .collect();
    let digits = std::str::from_utf8(&digits).unwrap_or("");
    let digits = digits.trim_start_matches('0');
    digits.len() > 10 || (digits.len() == 10 && digits > "2147483647")
}

fn check_line(line: &[u8]) {
    let c_line = CString::new(line).expect("no NUL in a generated line");
    let c = c_result(&c_line);
    let mut s = shim_result(&c_line);
    let mut r = core_result(line);
    if !cfg!(target_os = "linux") && c.value.is_some_and(|v| atoi_out_of_range(&line[v..])) {
        s.physical_id = c.physical_id;
        r.physical_id = c.physical_id;
    }
    assert_eq!(s, c, "shim vs C, line {line:?}");
    assert_eq!(r, c, "core vs C, line {line:?}");
}

/// Every line of up to three pieces, with and without a newline.
#[test]
fn every_line_of_three_pieces() {
    let n = PIECES.len();
    let mut count = 0usize;
    for len in 0..=3u32 {
        for mut i in 0..n.pow(len) {
            let mut line = Vec::new();
            for _ in 0..len {
                line.extend_from_slice(PIECES[i % n]);
                i /= n;
            }
            check_line(&line);
            line.push(b'\n');
            check_line(&line);
            count += 2;
        }
    }
    assert_eq!(count, 2 * (1 + n + n * n + n * n * n));
}

/// Every byte alone, and after and before each key and its colon.
#[test]
fn every_byte_after_each_key() {
    for b in 1..=255u8 {
        for prefix in [
            &b""[..],
            b"model name",
            b"model name\t: ",
            b"physical id\t:",
            b"cpu MHz\t\t: ",
            b"CPU: x (",
            b"CPU: x (1",
        ] {
            let mut line = prefix.to_vec();
            line.push(b);
            check_line(&line);
            line.push(b'\n');
            check_line(&line);
            let mut before = vec![b];
            before.extend_from_slice(prefix);
            check_line(&before);
        }
    }
}

/// This host's own /proc/cpuinfo, when it has one.
#[test]
fn host_cpuinfo_lines() {
    let Ok(text) = std::fs::read("/proc/cpuinfo") else {
        return;
    };
    for line in text.split_inclusive(|&b| b == b'\n') {
        if !line.contains(&0) {
            check_line(line);
        }
    }
}

fn check_atoi(text: &str) {
    let c_text = CString::new(text).expect("no NUL");
    // SAFETY: a live NUL-terminated string.
    let c = unsafe { oracle::oracle_obs_nix_system_info_physical_id(c_text.as_ptr()) };
    // SAFETY: as above.
    let s = unsafe { shim::obs_nix_system_info_physical_id(c_text.as_ptr()) };
    let r = core_rules::physical_id(text.as_bytes());
    if !cfg!(target_os = "linux") && atoi_out_of_range(text.as_bytes()) {
        return;
    }
    assert_eq!(s, c, "shim vs C, {text:?}");
    assert_eq!(r, c, "core vs C, {text:?}");
}

/// `atoi()` over every value from -70,000 to 70,000, the ends of `int` and
/// past them, with white space, signs and trailing text.
#[test]
fn every_physical_id() {
    for v in -70_000i64..=70_000 {
        check_atoi(&format!(" {v}\n"));
    }
    for v in [
        i64::from(i32::MIN) - 1,
        i64::from(i32::MIN),
        i64::from(i32::MIN) + 1,
        i64::from(i32::MAX) - 1,
        i64::from(i32::MAX),
        i64::from(i32::MAX) + 1,
        i64::MIN,
        i64::MAX,
    ] {
        for form in [
            format!("{v}"),
            format!(" \t{v}\n"),
            format!("{v}abc"),
            format!("0000{v}"),
        ] {
            check_atoi(&form);
        }
    }
    for text in [
        "",
        " ",
        "\n",
        "-",
        "+",
        "--1",
        "+-1",
        "-+1",
        " - 1",
        "\x0b\x0c\r 9",
        "0x10",
        "1e3",
        "١",
        "99999999999999999999",
        "-99999999999999999999",
        "18446744073709551616",
    ] {
        check_atoi(text);
    }
}

fn check_memory(total: u64, free: u64, buffer: u64, unit: u32, mem: i64) {
    // SAFETY: plain values.
    let c = unsafe {
        (
            oracle::oracle_obs_nix_system_info_total_mb(total, unit),
            oracle::oracle_obs_nix_system_info_free_mb(free, buffer, unit),
            oracle::oracle_obs_nix_system_info_physmem_mb(mem),
        )
    };
    let s = (
        shim::obs_nix_system_info_total_mb(total, unit),
        shim::obs_nix_system_info_free_mb(free, buffer, unit),
        shim::obs_nix_system_info_physmem_mb(mem),
    );
    let r = (
        core_rules::total_mb(total, unit),
        core_rules::free_mb(free, buffer, unit),
        core_rules::physmem_mb(mem),
    );
    assert_eq!(s, c, "shim vs C, {total} {free} {buffer} {unit} {mem}");
    assert_eq!(r, c, "core vs C, {total} {free} {buffer} {unit} {mem}");
}

/// The memory sizes over the edges of every field.
#[test]
fn every_memory_edge() {
    let sizes = [
        0u64,
        1,
        255,
        256,
        1_048_575,
        1_048_576,
        1_048_577,
        u64::from(u32::MAX),
        1 << 32,
        1 << 52,
        (1 << 63) - 1,
        1 << 63,
        u64::MAX - 1,
        u64::MAX,
    ];
    let units = [0u32, 1, 2, 4096, 65_536, u32::MAX - 1, u32::MAX];
    let mems = [
        0i64,
        1,
        -1,
        1_048_575,
        1_048_576,
        -1_048_575,
        -1_048_576,
        -1_048_577,
        -2_097_151,
        i64::MAX,
        i64::MIN,
        i64::MIN + 1,
    ];
    for &total in &sizes {
        for &free in &sizes {
            for &buffer in &sizes {
                for &unit in &units {
                    check_memory(total, free, buffer, unit, mems[free as usize % mems.len()]);
                }
            }
        }
    }
    for &mem in &mems {
        check_memory(0, 0, 0, 0, mem);
    }
}

/// One side's functions, for the callers' loops.
struct Side {
    is_name: unsafe fn(*const c_char) -> u8,
    is_physical_id: unsafe fn(*const c_char) -> u8,
    is_speed: unsafe fn(*const c_char) -> u8,
    is_dmesg_cpu: unsafe fn(*const c_char) -> u8,
    value: unsafe extern "C" fn(*const c_char) -> *const c_char,
    physical_id: unsafe extern "C" fn(*const c_char) -> c_int,
    prints: unsafe fn(*const c_char, c_int, c_int) -> u8,
    dmesg_speed: unsafe extern "C" fn(*const c_char, *mut usize) -> *const c_char,
}

const C_SIDE: Side = Side {
    is_name: o_is_name,
    is_physical_id: o_is_physical_id,
    is_speed: o_is_speed,
    is_dmesg_cpu: o_is_dmesg_cpu,
    value: oracle::oracle_obs_nix_system_info_value,
    physical_id: oracle::oracle_obs_nix_system_info_physical_id,
    prints: o_prints,
    dmesg_speed: oracle::oracle_obs_nix_system_info_dmesg_speed,
};

const SHIM_SIDE: Side = Side {
    is_name: s_is_name,
    is_physical_id: s_is_physical_id,
    is_speed: s_is_speed,
    is_dmesg_cpu: s_is_dmesg_cpu,
    value: shim::obs_nix_system_info_value,
    physical_id: shim::obs_nix_system_info_physical_id,
    prints: s_prints,
    dmesg_speed: shim::obs_nix_system_info_dmesg_speed,
};

/// What a caller would log, as `log_processor_info()` and
/// `log_processor_speed()` run: each printed block's name and speed (the
/// value after the C's cut, before its depad, which stays C), and the
/// dmesg.boot speed, with every step on one side's functions.
fn run_callers(side: &Side, text: &[u8]) -> Vec<String> {
    let mut log = Vec::new();
    let mut physical_id: c_int = -1;
    let mut last: c_int = -1;
    let mut name: Option<Vec<u8>> = None;
    let mut speed: Option<Vec<u8>> = None;
    let mut dmesg: Option<Vec<u8>> = None;
    // the value the C copies and cuts: its bytes before the NUL, less one
    let cut = |v: *const c_char| -> Vec<u8> {
        // SAFETY: a pointer into a live NUL-terminated line.
        let mut b = unsafe { std::ffi::CStr::from_ptr(v) }.to_bytes().to_vec();
        b.pop();
        b
    };
    for line in text.split_inclusive(|&b| b == b'\n') {
        let line = CString::new(line).expect("no NUL in a generated file");
        let p = line.as_ptr();
        // SAFETY: `p` is a live NUL-terminated string, and the pointers the
        // functions return point into it.
        unsafe {
            if (side.is_dmesg_cpu)(p) != 0 {
                let mut len = 0usize;
                let start = (side.dmesg_speed)(p, &mut len);
                if !start.is_null() {
                    dmesg = Some(std::slice::from_raw_parts(start.cast::<u8>(), len).to_vec());
                }
            }
            if (side.is_name)(p) != 0 {
                let start = (side.value)(p);
                if start.is_null() {
                    continue;
                }
                name = Some(cut(start));
            }
            if (side.is_physical_id)(p) != 0 {
                let start = (side.value)(p);
                if start.is_null() {
                    continue;
                }
                physical_id = (side.physical_id)(start);
            }
            if (side.is_speed)(p) != 0 {
                let start = (side.value)(p);
                if start.is_null() {
                    continue;
                }
                speed = Some(cut(start));
            }
            if (side.prints)(p, physical_id, last) != 0 {
                last = physical_id;
                log.push(format!("block {name:?} {speed:?}"));
            }
        }
    }
    log.push(format!("end {name:?} {speed:?} {physical_id} {dmesg:?}"));
    log
}

/// Whether a file has a `physical id` line whose value is out of `int`
/// range, which only Linux compares (see the module docs).
fn has_out_of_range_id(text: &[u8]) -> bool {
    text.split_inclusive(|&b| b == b'\n').any(|line| {
        core_rules::is_physical_id(line)
            && core_rules::value(line).is_some_and(|v| atoi_out_of_range(&line[v..]))
    })
}

fn check_callers(text: &[u8]) {
    if !cfg!(target_os = "linux") && has_out_of_range_id(text) {
        return;
    }
    let c = run_callers(&C_SIDE, text);
    let s = run_callers(&SHIM_SIDE, text);
    assert_eq!(s, c, "callers, text {:?}", String::from_utf8_lossy(text));
}

/// The callers over the C test's texts.
#[test]
fn callers_over_saved_text() {
    for text in [
        &b"processor\t: 0\nmodel name\t: Intel(R) Xeon(R) CPU E5-2680 v4 @ 2.40GHz\ncpu MHz\t\t: 2394.454\nphysical id\t: 0\n\nprocessor\t: 1\nmodel name\t: Intel(R) Xeon(R) CPU E5-2690 v4 @ 2.60GHz\ncpu MHz\t\t: 2600.000\nphysical id\t: 1\n\n"[..],
        b"processor\t: 0\nBogoMIPS\t: 48.00\nCPU implementer\t: 0x41\n\nprocessor\t: 1\n\n",
        b"physical id\t: 0\n\nmodel name\t: Late\nphysical id\t: 1\ncpu MHz\t\t: 1.0\n\n",
        b"physical id\t: 0\ncpu MHz\t\t: 1234.567\nmodel name\t: Name",
        b"model name\t: Old\nmodel name\t:\nphysical id\t: 0\n\n",
        b"model name\t: Old\nmodel name\t:",
        b"physical id : x\nmodel name : X\n\n",
        b"Copyright (c) 1992-2023 The FreeBSD Project.\nCPU: AMD Ryzen 7 5800X 8-Core Processor (3800.11-MHz K8-class CPU)\n",
        b"CPU: A (1000.00-MHz)\nCPU: B (2000.00-MHz)\nCPU: none\nCPU: x (\n",
    ] {
        check_callers(text);
    }
    if let Ok(text) = std::fs::read("/proc/cpuinfo")
        && !text.contains(&0)
    {
        check_callers(&text);
    }
}

fn line_strategy() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(
        prop_oneof![
            4 => prop::sample::select(PIECES).prop_map(<[u8]>::to_vec),
            1 => (1..=255u8).prop_map(|b| vec![b]),
        ],
        0..10,
    )
    .prop_map(|p| p.concat())
}

/// The starts of a generated file's lines.
const KEYS: &[&[u8]] = &[
    b"model name\t: ",
    b"physical id\t: ",
    b"cpu MHz\t\t: ",
    b"CPU: x (",
    b"",
];

fn file_strategy() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(
        prop_oneof![
            2 => Just(b"\n".to_vec()),
            3 => (
                prop::sample::select(KEYS),
                line_strategy()
            )
                .prop_map(|(k, l)| [k, &l[..], b"\n"].concat()),
        ],
        0..24,
    )
    .prop_map(|lines| lines.concat())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    /// Every line rule over any line of pieces and bytes.
    #[test]
    fn proptest_lines(line in line_strategy()) {
        check_line(&line);
    }

    /// `atoi()` over any number text.
    #[test]
    fn proptest_physical_ids(
        space in "[ \t\n\x0b\x0c\r]{0,3}",
        sign in prop::sample::select(&["", "-", "+", "--"][..]),
        digits in "[0-9]{0,22}",
        tail in "[0-9a-z: \n-]{0,4}",
    ) {
        check_atoi(&format!("{space}{sign}{digits}{tail}"));
    }

    /// The memory sizes over any fields.
    #[test]
    fn proptest_memory(
        total in any::<u64>(),
        free in any::<u64>(),
        buffer in any::<u64>(),
        unit in any::<u32>(),
        mem in any::<i64>(),
    ) {
        check_memory(total, free, buffer, unit, mem);
    }

    /// Both callers over any generated file.
    #[test]
    fn proptest_callers(text in file_strategy()) {
        check_callers(&text);
    }
}

/// The oracle's NULL, so a test can see the shim and the C agree on it.
#[test]
fn null_results() {
    let line = CString::new("CPU: none\n").expect("no NUL");
    let mut len = 7usize;
    // SAFETY: a live NUL-terminated string and a writable size_t.
    unsafe {
        assert_eq!(
            oracle::oracle_obs_nix_system_info_dmesg_speed(line.as_ptr(), &mut len),
            ptr::null()
        );
        assert_eq!(len, 7);
        assert_eq!(
            shim::obs_nix_system_info_dmesg_speed(line.as_ptr(), &mut len),
            ptr::null()
        );
        assert_eq!(len, 7);
        let line = CString::new("model name\n").expect("no NUL");
        assert_eq!(
            oracle::oracle_obs_nix_system_info_value(line.as_ptr()),
            ptr::null()
        );
        assert_eq!(shim::obs_nix_system_info_value(line.as_ptr()), ptr::null());
    }
}
