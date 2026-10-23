//! `libobs/util/platform-conversions.c` (#262 slice 1): the obs-util C ABI
//! shims against the original C. Text is mixed ASCII and 2-, 3- and 4-byte
//! UTF-8, as file names and source names are. The `_ptr` timings include
//! freeing the result with its own side's `bfree`. The mbs case runs in the
//! C locale on ASCII, where both sides make the same C library calls.

use std::ffi::c_char;
use std::hint::black_box;
use std::ptr;

use criterion::{BenchmarkId, Criterion, Throughput};
use obs_c_oracle::bmem as c_mem;
use obs_c_oracle::platform_conversions::{self as c, WChar as CWChar};
use obs_util::ffi::bmem as rs_mem;
use obs_util::ffi::platform_conversions as rs;
use obs_util::platform_conversions::WChar;

/// `size` bytes of mixed UTF-8 (cut on a character boundary), with a NUL.
fn utf8_text(size: usize) -> Vec<u8> {
    let mut text = String::new();
    for c in "obs scene \u{E9}t\u{E9} \u{20AC}5 \u{1F600} "
        .chars()
        .cycle()
    {
        if text.len() + c.len_utf8() > size {
            break;
        }
        text.push(c);
    }
    let mut bytes = text.into_bytes();
    bytes.push(0);
    bytes
}

fn wide_of(text: &[u8]) -> (Vec<WChar>, Vec<CWChar>) {
    let s = text.as_ptr().cast::<c_char>();
    // SAFETY: `text` is NUL-terminated.
    let n = unsafe { rs::os_utf8_to_wcs(s, 0, ptr::null_mut(), 0) };
    let mut r: Vec<WChar> = vec![0; n + 1];
    let mut k: Vec<CWChar> = vec![0; n + 1];
    // SAFETY: both buffers hold `n + 1` units.
    unsafe {
        rs::os_utf8_to_wcs(s, 0, r.as_mut_ptr(), n + 1);
        c::oracle_os_utf8_to_wcs(s, 0, k.as_mut_ptr(), n + 1);
    }
    (r, k)
}

fn buffers(criterion: &mut Criterion) {
    for size in [64, 4096] {
        let text = utf8_text(size);
        let s = text.as_ptr().cast::<c_char>();
        let (wr, wc) = wide_of(&text);
        let units = wr.len();

        let mut group = criterion.benchmark_group("conversions/utf8_to_wcs");
        group.throughput(Throughput::Bytes(text.len() as u64 - 1));
        let mut rb: Vec<WChar> = vec![0; units];
        let mut cb: Vec<CWChar> = vec![0; units];
        // SAFETY: NUL-terminated input; both buffers hold `units` units.
        unsafe {
            assert_eq!(
                rs::os_utf8_to_wcs(s, 0, rb.as_mut_ptr(), units),
                c::oracle_os_utf8_to_wcs(s, 0, cb.as_mut_ptr(), units)
            );
        }
        group.bench_function(BenchmarkId::new("rust", size), |b| {
            // SAFETY: as above.
            b.iter(|| unsafe { rs::os_utf8_to_wcs(black_box(s), 0, rb.as_mut_ptr(), units) })
        });
        group.bench_function(BenchmarkId::new("c", size), |b| {
            // SAFETY: as above.
            b.iter(|| unsafe { c::oracle_os_utf8_to_wcs(black_box(s), 0, cb.as_mut_ptr(), units) })
        });
        group.finish();

        let mut group = criterion.benchmark_group("conversions/wcs_to_utf8");
        group.throughput(Throughput::Bytes(text.len() as u64 - 1));
        let mut rb = vec![0u8; text.len()];
        let mut cb = vec![0u8; text.len()];
        let n = text.len();
        // SAFETY: NUL-terminated wide input; both buffers hold `n` bytes.
        unsafe {
            assert_eq!(
                rs::os_wcs_to_utf8(wr.as_ptr(), 0, rb.as_mut_ptr().cast(), n),
                c::oracle_os_wcs_to_utf8(wc.as_ptr(), 0, cb.as_mut_ptr().cast(), n)
            );
        }
        assert_eq!(rb, cb);
        group.bench_function(BenchmarkId::new("rust", size), |b| {
            // SAFETY: as above.
            b.iter(|| unsafe {
                rs::os_wcs_to_utf8(black_box(wr.as_ptr()), 0, rb.as_mut_ptr().cast(), n)
            })
        });
        group.bench_function(BenchmarkId::new("c", size), |b| {
            // SAFETY: as above.
            b.iter(|| unsafe {
                c::oracle_os_wcs_to_utf8(black_box(wc.as_ptr()), 0, cb.as_mut_ptr().cast(), n)
            })
        });
        group.finish();
    }
}

fn pointers(criterion: &mut Criterion) {
    let text = utf8_text(64);
    let s = text.as_ptr().cast::<c_char>();
    let ascii = c"obs-recording-2026-10-22_12-00-00.mkv";

    // SAFETY (all calls): NUL-terminated inputs; each result is freed with
    // its own side's `bfree`.
    super::pair(
        criterion,
        "conversions/utf8_to_wcs_ptr/64",
        || unsafe {
            let mut p: *mut WChar = ptr::null_mut();
            let n = rs::os_utf8_to_wcs_ptr(black_box(s), 0, &mut p);
            rs_mem::bfree(p.cast());
            n
        },
        || unsafe {
            let mut p: *mut CWChar = ptr::null_mut();
            let n = c::oracle_os_utf8_to_wcs_ptr(black_box(s), 0, &mut p);
            c_mem::oracle_bfree(p.cast());
            n
        },
    );
    super::pair(
        criterion,
        "conversions/utf8_to_mbs_ptr/ascii",
        || unsafe {
            let mut p: *mut c_char = ptr::null_mut();
            let n = rs::os_utf8_to_mbs_ptr(black_box(ascii.as_ptr()), 0, &mut p);
            rs_mem::bfree(p.cast());
            n
        },
        || unsafe {
            let mut p: *mut c_char = ptr::null_mut();
            let n = c::oracle_os_utf8_to_mbs_ptr(black_box(ascii.as_ptr()), 0, &mut p);
            c_mem::oracle_bfree(p.cast());
            n
        },
    );
}

pub fn benchmarks(criterion: &mut Criterion) {
    buffers(criterion);
    pointers(criterion);
}
