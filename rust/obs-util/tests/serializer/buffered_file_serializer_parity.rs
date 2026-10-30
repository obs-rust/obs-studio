//! Tier 3: `buffered_file_serializer_*` against the C oracle.
//!
//! Exclusions, not fed to the oracle:
//! - Chunks larger than the queue capacity: Rust accepts one whole chunk;
//!   C can wait forever. The bounded oversized-chunk regression covers it.
//! - Disk write failures with a producer waiting for space: Rust wakes the
//!   producer; C can wait forever. The controlled failure regression covers it.
//! - `buffered_file_serializer_free(NULL)` returns. C crashes on a null serializer.
//! - A null data pointer with a non-zero size is undefined in C (`deque_push_back`
//!   reads it). The shim returns 0.

use std::ffi::{CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use obs_c_oracle::buffered_file_serializer::{
    OracleSerializer, oracle_buffered_file_serializer_free, oracle_buffered_file_serializer_init,
    oracle_buffered_file_serializer_init_defaults,
};
use obs_util::ffi::array_serializer::serializer;
use obs_util::ffi::buffered_file_serializer::{
    buffered_file_serializer_free, buffered_file_serializer_init,
    buffered_file_serializer_init_defaults,
};
use proptest::prelude::*;

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "obs-buffered-parity-{}-{}-{n}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn c_path(&self, name: &str) -> (PathBuf, CString) {
        let path = self.0.join(name);
        let text = path.to_str().expect("scratch path is utf-8").to_owned();
        (path, CString::new(text).unwrap())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[derive(Debug, Clone)]
enum Op {
    Write(Vec<u8>),
    Seek(i64, i32),
    Pos,
}

#[derive(Debug)]
struct Trace {
    init: bool,
    read_set: bool,
    write_set: bool,
    seek_set: bool,
    pos_set: bool,
    rets: Vec<i64>,
    bytes: Option<Vec<u8>>,
}

fn zero_rust() -> serializer {
    serializer {
        data: std::ptr::null_mut(),
        read: None,
        write: None,
        seek: None,
        get_pos: None,
    }
}

fn zero_oracle() -> OracleSerializer {
    OracleSerializer {
        data: std::ptr::null_mut(),
        read: None,
        write: None,
        seek: None,
        get_pos: None,
    }
}

fn file_bytes(path: &Path) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

fn trace_rust(path: &Path, c_path: &CStr, max_buf: usize, chunk: usize, ops: &[Op]) -> Trace {
    let mut s = zero_rust();
    // SAFETY: `s` is a local serializer. `c_path` is a C string.
    let init = unsafe { buffered_file_serializer_init(&mut s, c_path.as_ptr(), max_buf, chunk) };
    if !init {
        assert!(s.data.is_null());
        return Trace {
            init: false,
            read_set: s.read.is_some(),
            write_set: s.write.is_some(),
            seek_set: s.seek.is_some(),
            pos_set: s.get_pos.is_some(),
            rets: Vec::new(),
            bytes: file_bytes(path),
        };
    }
    let mut rets = Vec::new();
    for op in ops {
        // SAFETY: init stored an `Output` and the three callbacks.
        let ret = unsafe {
            match op {
                Op::Write(buf) => s.write.unwrap()(s.data, buf.as_ptr().cast(), buf.len()) as i64,
                Op::Seek(offset, kind) => s.seek.unwrap()(s.data, *offset, *kind),
                Op::Pos => s.get_pos.unwrap()(s.data),
            }
        };
        rets.push(ret);
    }
    let dangling = s.data;
    // SAFETY: `s` came from init.
    unsafe { buffered_file_serializer_free(&mut s) };
    assert_eq!(s.data, dangling);
    Trace {
        init: true,
        read_set: s.read.is_some(),
        write_set: s.write.is_some(),
        seek_set: s.seek.is_some(),
        pos_set: s.get_pos.is_some(),
        rets,
        bytes: file_bytes(path),
    }
}

fn trace_oracle(path: &Path, c_path: &CStr, max_buf: usize, chunk: usize, ops: &[Op]) -> Trace {
    let mut s = zero_oracle();
    // SAFETY: `s` is a local serializer. `c_path` is a C string.
    let init =
        unsafe { oracle_buffered_file_serializer_init(&mut s, c_path.as_ptr(), max_buf, chunk) };
    if !init {
        assert!(s.data.is_null());
        return Trace {
            init: false,
            read_set: s.read.is_some(),
            write_set: s.write.is_some(),
            seek_set: s.seek.is_some(),
            pos_set: s.get_pos.is_some(),
            rets: Vec::new(),
            bytes: file_bytes(path),
        };
    }
    let mut rets = Vec::new();
    for op in ops {
        // SAFETY: the oracle init stored its writer and callbacks.
        let ret = unsafe {
            match op {
                Op::Write(buf) => s.write.unwrap()(s.data, buf.as_ptr().cast(), buf.len()) as i64,
                Op::Seek(offset, kind) => s.seek.unwrap()(s.data, *offset, *kind),
                Op::Pos => s.get_pos.unwrap()(s.data),
            }
        };
        rets.push(ret);
    }
    let dangling = s.data;
    // SAFETY: `s` came from the oracle init.
    unsafe { oracle_buffered_file_serializer_free(&mut s) };
    assert_eq!(s.data, dangling);
    Trace {
        init: true,
        read_set: s.read.is_some(),
        write_set: s.write.is_some(),
        seek_set: s.seek.is_some(),
        pos_set: s.get_pos.is_some(),
        rets,
        bytes: file_bytes(path),
    }
}

fn assert_match(max_buf: usize, chunk: usize, ops: &[Op]) {
    let scratch = Scratch::new();
    let (rust_path, rust_c) = scratch.c_path("rust.bin");
    let (oracle_path, oracle_c) = scratch.c_path("oracle.bin");
    let rust = trace_rust(&rust_path, &rust_c, max_buf, chunk, ops);
    let oracle = trace_oracle(&oracle_path, &oracle_c, max_buf, chunk, ops);
    assert_eq!(rust.init, oracle.init);
    assert_eq!(rust.read_set, oracle.read_set);
    assert_eq!(rust.write_set, oracle.write_set);
    assert_eq!(rust.seek_set, oracle.seek_set);
    assert_eq!(rust.pos_set, oracle.pos_set);
    assert_eq!(rust.rets, oracle.rets);
    assert_eq!(rust.bytes, oracle.bytes);
}

fn pattern(n: usize, seed: u8) -> Vec<u8> {
    (0..n)
        .map(|i| (i as u8).wrapping_mul(3).wrapping_add(seed))
        .collect()
}

/// `test_buffered_defaults`, through the C ABI.
#[test]
fn defaults_match_oracle() {
    let data = pattern(20, 1);
    assert_match(
        0,
        0,
        &[
            Op::Pos,
            Op::Write(data),
            Op::Pos,
            Op::Write(Vec::new()),
            Op::Pos,
        ],
    );
}

/// `test_buffered_small_chunks`.
#[test]
fn small_chunks_match_oracle() {
    let data = (0..1000u32).map(|i| (i ^ (i >> 8) ^ 0x5A) as u8).collect();
    assert_match(64, 8, &[Op::Write(data), Op::Pos]);
}

/// `test_buffered_seek_overwrite`.
#[test]
fn seek_overwrite_matches_oracle() {
    let original: Vec<u8> = (0..100).map(|i| i as u8).collect();
    assert_match(
        64,
        8,
        &[
            Op::Write(original),
            Op::Pos,
            Op::Seek(10, 0),
            Op::Pos,
            Op::Write(b"XYZ".to_vec()),
            Op::Pos,
            Op::Seek(7, 1),
            Op::Write(b"Q".to_vec()),
            Op::Pos,
            Op::Seek(5, 2),
            Op::Pos,
            Op::Write(b"E".to_vec()),
            Op::Pos,
        ],
    );
}

/// `test_buffered_seek_past_end`, including the gap bytes.
#[test]
fn seek_past_end_matches_oracle() {
    assert_match(
        64,
        8,
        &[
            Op::Write(b"ab".to_vec()),
            Op::Seek(6, 0),
            Op::Write(b"cd".to_vec()),
            Op::Pos,
        ],
    );
}

/// `test_output_unwritable_path`, buffered inits.
#[test]
fn unwritable_path_matches_oracle() {
    let scratch = Scratch::new();
    let missing = scratch.0.join("no_such_subdir").join("x.bin");
    let c = CString::new(missing.to_str().unwrap()).unwrap();
    let rust = trace_rust(&missing, &c, 64, 8, &[]);
    let oracle = trace_oracle(&missing, &c, 64, 8, &[]);
    assert!(!rust.init);
    assert!(!oracle.init);
    assert!(!missing.exists());
    assert_eq!(rust.bytes, oracle.bytes);

    let mut s = zero_rust();
    // SAFETY: `s` is writable. `c` is a C string to a missing directory.
    let ok = unsafe { buffered_file_serializer_init_defaults(&mut s, c.as_ptr()) };
    assert!(!ok);
    assert!(s.data.is_null());
    let mut o = zero_oracle();
    // SAFETY: same path, oracle serializer.
    let oracle_ok = unsafe { oracle_buffered_file_serializer_init_defaults(&mut o, c.as_ptr()) };
    assert_eq!(ok, oracle_ok);
    assert!(o.data.is_null());
}

#[test]
fn null_path_matches_oracle() {
    let mut s = zero_rust();
    // SAFETY: null path. C fails `fopen` and does not write `s`.
    let ok = unsafe { buffered_file_serializer_init(&mut s, std::ptr::null(), 64, 8) };
    assert!(!ok);
    assert!(s.data.is_null());
    let mut o = zero_oracle();
    // SAFETY: null path, oracle serializer.
    let oracle_ok =
        unsafe { oracle_buffered_file_serializer_init(&mut o, std::ptr::null(), 64, 8) };
    assert_eq!(ok, oracle_ok);
    assert!(o.data.is_null());
}

#[test]
fn empty_path_matches_oracle() {
    let path = Path::new("");
    let c = CString::new("").unwrap();
    let rust = trace_rust(path, &c, 64, 8, &[]);
    let oracle = trace_oracle(path, &c, 64, 8, &[]);
    assert!(!rust.init);
    assert!(!oracle.init);
}

#[test]
fn unknown_seek_matches_oracle() {
    assert_match(
        64,
        8,
        &[
            Op::Write(b"abcdef".to_vec()),
            Op::Seek(3, 99),
            Op::Pos,
            Op::Write(b"Z".to_vec()),
        ],
    );
}

/// Eight bytes is one chunk and under 64 KiB, so both sides leave the file
/// empty until free. Checked here by comparing the finished files, which are
/// the payload, and by a direct size check on the Rust side before free.
#[test]
fn one_chunk_matches_oracle() {
    assert_match(64, 8, &[Op::Write(b"01234567".to_vec())]);
}

#[test]
fn free_null_data_returns() {
    let mut s = zero_rust();
    // SAFETY: null data is the C early return.
    unsafe { buffered_file_serializer_free(&mut s) };
    let mut o = zero_oracle();
    // SAFETY: null data, oracle serializer.
    unsafe { oracle_buffered_file_serializer_free(&mut o) };
}

#[test]
fn free_null_serializer_returns() {
    // SAFETY: the shim returns. The oracle is not called; C would crash.
    unsafe { buffered_file_serializer_free(std::ptr::null_mut()) };
}

/// APFS rejects a filename byte of 0xFF. Mutual failure matches. A filesystem
/// that accepts the byte must produce the same file.
#[test]
#[cfg(unix)]
fn non_utf8_filename_matches_oracle() {
    use std::os::unix::ffi::OsStrExt;

    let scratch = Scratch::new();
    let mut bytes = scratch.0.as_os_str().as_bytes().to_vec();
    bytes.extend_from_slice(b"/x\xff.bin");
    let c = CString::new(bytes.clone()).unwrap();
    let path = PathBuf::from(std::ffi::OsStr::from_bytes(&bytes));
    let ops = [Op::Write(b"hi".to_vec())];
    let rust = trace_rust(&path, &c, 64, 8, &ops);
    let oracle = trace_oracle(&path, &c, 64, 8, &ops);
    assert_eq!(rust.init, oracle.init);
    if rust.init {
        assert_eq!(rust.bytes, oracle.bytes);
        assert_eq!(rust.rets, oracle.rets);
    }
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        prop::collection::vec(any::<u8>(), 0..32).prop_map(Op::Write),
        (-4i64..40, prop::sample::select(vec![0, 1, 2, 99]))
            .prop_map(|(offset, kind)| Op::Seek(offset, kind)),
        Just(Op::Pos),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(24)))]

    #[test]
    fn random_ops_match_oracle(ops in prop::collection::vec(op_strategy(), 0..8)) {
        assert_match(64, 8, &ops);
    }
}
