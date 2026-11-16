//! Tier 3: the extracted C wrapper and real private-layout adapter, no GPU.
//! Counts are bounded to 32 (128 input bytes). Index types include values
//! outside `enum gs_index_type`, which take the 4-byte width through the C
//! ABI. No intentional differences.
use obs_c_oracle as _;
use obs_graphics::ffi::indexbuffer_create::gs_indexbuffer_create;
use obs_graphics::indexbuffer_create::DUP_BUFFER;
use obs_util::ffi::bmem::{bfree, bmemdup};
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_void};

unsafe extern "C" {
    fn oracle_gs_indexbuffer_create(
        kind: i32,
        input: *mut c_void,
        count: usize,
        flags: u32,
    ) -> *mut c_void;
    fn oracle_indexbuffer_reset(present: bool, reject: bool);
    fn oracle_indexbuffer_calls() -> usize;
    fn oracle_indexbuffer_input() -> *mut c_void;
    fn oracle_indexbuffer_type() -> i32;
    fn oracle_indexbuffer_count() -> usize;
    fn oracle_indexbuffer_flags() -> u32;
    fn oracle_indexbuffer_device_matches() -> bool;
    fn oracle_indexbuffer_logs() -> u32;
    fn oracle_indexbuffer_level() -> i32;
    fn oracle_indexbuffer_log() -> *const c_char;
    fn oracle_indexbuffer_destroy();
}

#[derive(Debug, PartialEq)]
struct Observed {
    returned: bool,
    calls: usize,
    kind: i32,
    count: usize,
    flags: u32,
    device: bool,
    duplicated: bool,
    bytes: Vec<u8>,
    independent: bool,
    logs: u32,
    level: i32,
    log: Vec<u8>,
}

fn run(rust: bool, state: (bool, bool, bool), kind: i32, flags: u32, bytes: &[u8]) -> Observed {
    let (context, present, reject) = state;
    let count = bytes.len() / if kind == 0 { 2 } else { 4 };
    let size = count * if kind == 0 { 2 } else { 4 };
    let storage = if size == 0 {
        &[0x37][..]
    } else {
        &bytes[..size]
    };
    // SAFETY: both implementations receive exact-sized libobs allocations;
    // the thread-local fake backend consumes the transferred allocation even
    // on failure. Only the untransferred original is freed separately.
    unsafe {
        let input = if present {
            bmemdup(storage.as_ptr().cast(), storage.len())
        } else {
            std::ptr::null_mut()
        };
        oracle_indexbuffer_reset(context, reject);
        let result = if rust {
            gs_indexbuffer_create(kind, input, count, flags)
        } else {
            oracle_gs_indexbuffer_create(kind, input, count, flags)
        };
        let transferred = oracle_indexbuffer_input();
        let copied = context && present && count != 0 && flags & DUP_BUFFER != 0;
        let data = if !transferred.is_null() && size != 0 {
            std::slice::from_raw_parts(transferred.cast::<u8>(), size).to_vec()
        } else {
            vec![]
        };
        let duplicated = !transferred.is_null() && transferred != input;
        let mut independent = false;
        if copied && !reject {
            std::ptr::write_bytes(input, 0, size);
            independent = std::slice::from_raw_parts(transferred.cast::<u8>(), size) == data;
        }
        let observed = Observed {
            returned: !result.is_null(),
            calls: oracle_indexbuffer_calls(),
            kind: oracle_indexbuffer_type(),
            count: oracle_indexbuffer_count(),
            flags: oracle_indexbuffer_flags(),
            device: oracle_indexbuffer_device_matches(),
            duplicated,
            bytes: data,
            independent,
            logs: oracle_indexbuffer_logs(),
            level: oracle_indexbuffer_level(),
            log: CStr::from_ptr(oracle_indexbuffer_log()).to_bytes().to_vec(),
        };
        if context {
            oracle_indexbuffer_destroy();
        }
        if !context || copied {
            if present && (reject || !context) {
                assert_eq!(
                    std::slice::from_raw_parts(input.cast::<u8>(), storage.len()),
                    storage
                );
            }
            bfree(input);
        }
        observed
    }
}

fn compare(context: bool, present: bool, reject: bool, kind: i32, flags: u32, bytes: &[u8]) {
    let original = run(false, (context, present, reject), kind, flags, bytes);
    let rust = run(true, (context, present, reject), kind, flags, bytes);
    assert_eq!(rust, original);
    assert_eq!(rust.calls, usize::from(context));
    assert_eq!(rust.returned, context && !reject);
    if context {
        assert!(rust.device);
        assert_eq!((rust.kind, rust.flags), (kind, flags));
        assert_eq!(rust.logs, 0);
    } else {
        assert_eq!(rust.logs, 1);
        assert_eq!(rust.level, 400);
        assert_eq!(
            rust.log,
            b"gs_indexbuffer_create: called while not in a graphics context"
        );
    }
}

#[test]
fn characterized_matrix_matches_c() {
    for context in [false, true] {
        for present in [false, true] {
            for reject in [false, true] {
                for kind in [0, 1, 2, -1, i32::MIN, i32::MAX] {
                    for flags in [0, 2, DUP_BUFFER, DUP_BUFFER | 2, u32::MAX] {
                        for bytes in [&[][..], &b"abcdefghijkl"[..]] {
                            compare(context, present, reject, kind, flags, bytes);
                        }
                    }
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default()
    })]
    #[test]
    fn random_input_matches_c(
        context in any::<bool>(), present in any::<bool>(), reject in any::<bool>(),
        kind in prop_oneof![0i32..=1, any::<i32>()], flags in any::<u32>(), bytes in prop::collection::vec(any::<u8>(), 0..=128),
    ) {
        compare(context, present, reject, kind, flags, &bytes);
    }
}
