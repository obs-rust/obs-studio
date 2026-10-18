//! The AVC and HEVC header extraction shims allocate only their C-owned
//! outputs (#1302).
//!
//! `obs_extract_avc_headers` and `obs_extract_hevc_headers` grew three
//! `Vec`s (packet, header, SEI) unit by unit and then copied each into
//! `bmalloc`, which made `avc/extract/256` and `hevc/extract/256` about
//! 1.25x slower than C. For a normal packet the shims must make no Rust
//! heap allocation; the counter (`alloc_count.rs`) does not see the
//! `bmalloc` outputs themselves.

use core::ptr;

use obs_codec::ffi::{avc, hevc};
use obs_util::ffi::darray::bfree;

use super::alloc_count::allocs_during;

type Extract = unsafe extern "C" fn(
    *const u8,
    usize,
    *mut *mut u8,
    *mut usize,
    *mut *mut u8,
    *mut usize,
    *mut *mut u8,
    *mut usize,
);

/// Runs `extract` on `data`; returns the Rust heap allocations and the
/// three output sizes.
fn extract(f: Extract, data: &[u8]) -> (usize, [usize; 3]) {
    let mut p = [ptr::null_mut(); 3];
    let mut n = [0usize; 3];
    let [p0, p1, p2] = &mut p;
    let [n0, n1, n2] = &mut n;
    let allocs = allocs_during(|| {
        // SAFETY: `data` is readable for its length; the outputs are locals.
        unsafe { f(data.as_ptr(), data.len(), p0, n0, p1, n1, p2, n2) };
    });
    for out in p {
        // SAFETY: each output came from bmalloc (or is NULL) and is freed once.
        unsafe { bfree(out.cast()) };
    }
    (allocs, n)
}

fn unit(header: &[u8], payload_len: usize) -> Vec<u8> {
    let mut v = vec![0, 0, 0, 1];
    v.extend_from_slice(header);
    v.extend(std::iter::repeat_n(0x55, payload_len));
    v
}

/// An AVC keyframe: SPS, PPS, SEI, then IDR slices up to about `size`.
fn avc_packet(size: usize, slice_len: usize) -> Vec<u8> {
    let mut p = [unit(&[0x67], 12), unit(&[0x68], 4), unit(&[0x06], 20)].concat();
    while p.len() < size {
        p.extend(unit(&[0x65], slice_len));
    }
    p
}

/// An HEVC IRAP picture: VPS, SPS, PPS, prefix SEI, then IDR slices.
fn hevc_packet(size: usize, slice_len: usize) -> Vec<u8> {
    let nal = |kind: u8| [kind << 1, 1];
    let mut p = [
        unit(&nal(32), 16),
        unit(&nal(33), 24),
        unit(&nal(34), 6),
        unit(&nal(39), 20),
    ]
    .concat();
    while p.len() < size {
        p.extend(unit(&nal(19), slice_len));
    }
    p
}

#[test]
fn avc_extract_allocates_only_the_outputs() {
    for size in [256, 16384] {
        let data = avc_packet(size, 160);
        let (allocs, n) = extract(avc::obs_extract_avc_headers, &data);
        assert!(n.iter().all(|&x| x > 0), "every output has data: {n:?}");
        assert_eq!(n.iter().sum::<usize>(), data.len());
        assert_eq!(
            allocs, 0,
            "Rust heap allocations for a {size}-byte AVC packet"
        );
    }
}

#[test]
fn hevc_extract_allocates_only_the_outputs() {
    for size in [256, 16384] {
        let data = hevc_packet(size, 160);
        let (allocs, n) = extract(hevc::obs_extract_hevc_headers, &data);
        assert!(n.iter().all(|&x| x > 0), "every output has data: {n:?}");
        assert_eq!(n.iter().sum::<usize>(), data.len());
        assert_eq!(
            allocs, 0,
            "Rust heap allocations for a {size}-byte HEVC packet"
        );
    }
}

#[test]
fn extract_without_start_code_allocates_nothing() {
    let (allocs, n) = extract(avc::obs_extract_avc_headers, &[0x55; 64]);
    assert_eq!((allocs, n), (0, [0, 0, 0]));
}

#[test]
fn extract_with_very_many_units_allocates_at_most_once() {
    // More units than any encoder writes in one packet: the shim may fall
    // back to one heap buffer for the unit list, never one per unit.
    let data = avc_packet(64 * 1024, 8);
    let (allocs, n) = extract(avc::obs_extract_avc_headers, &data);
    assert_eq!(n.iter().sum::<usize>(), data.len());
    assert!(
        allocs <= 1,
        "{allocs} Rust heap allocations for {} units",
        data.len() / 13
    );
}
