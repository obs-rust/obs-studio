//! The AV1 output shims allocate only their C-owned outputs (#1304).
//!
//! `obs-av1.c` writes the packet, header and metadata OBUs straight into
//! their `bmalloc` buffers. The shims must do the same: no Rust heap
//! allocation on the way (an intermediate `Vec` that grows and is then
//! copied into `bmalloc` made `av1/extract` and `av1/metadata` 1.26x to
//! 1.51x slower than C). The counter (`alloc_count.rs`) does not see the
//! `bmalloc` outputs themselves; counting is deterministic, unlike a time
//! bound.

use core::ptr;

use obs_codec::av1::{OBU_FRAME, OBU_METADATA, OBU_SEQUENCE_HEADER, OBU_TEMPORAL_DELIMITER};
use obs_codec::ffi::av1 as rs;
use obs_util::ffi::darray::bfree;

use super::alloc_count::allocs_during;

fn obu(kind: u8, payload: &[u8]) -> Vec<u8> {
    let mut v = vec![(kind << 3) | (1 << 1), payload.len() as u8];
    v.extend_from_slice(payload);
    v
}

/// A temporal unit like an encoder's keyframe: TD, sequence header,
/// metadata, then frames up to about `size` bytes.
fn packet(size: usize) -> Vec<u8> {
    let mut p = obu(OBU_TEMPORAL_DELIMITER, &[]);
    p.extend(obu(OBU_SEQUENCE_HEADER, &[0x0a; 12]));
    p.extend(obu(OBU_METADATA, &[0x04; 20]));
    while p.len() < size {
        p.extend(obu(OBU_FRAME, &[0x10; 120]));
    }
    p
}

fn extract(data: &[u8]) -> (usize, usize) {
    let (mut np, mut ns, mut hp, mut hs) = (ptr::null_mut(), 0, ptr::null_mut(), 0);
    let n = allocs_during(|| {
        // SAFETY: `data` is readable for its length; the outputs are locals.
        unsafe {
            rs::obs_extract_av1_headers(
                data.as_ptr(),
                data.len(),
                &mut np,
                &mut ns,
                &mut hp,
                &mut hs,
            )
        };
    });
    // SAFETY: both outputs came from bmalloc (or are NULL) and are freed once.
    unsafe {
        bfree(np.cast());
        bfree(hp.cast());
    }
    (n, ns + hs)
}

#[test]
fn extract_headers_allocates_only_the_outputs() {
    for size in [256, 16384] {
        let data = packet(size);
        let (allocs, out) = extract(&data);
        assert!(
            out > data.len(),
            "the header is a copy of part of the packet"
        );
        assert_eq!(allocs, 0, "Rust heap allocations for a {size}-byte packet");
    }
}

#[test]
fn extract_headers_without_header_obus_allocates_nothing() {
    let data = [
        obu(OBU_TEMPORAL_DELIMITER, &[]),
        obu(OBU_FRAME, &[0x10; 40]),
    ]
    .concat();
    assert_eq!(extract(&data).0, 0);
}

#[test]
fn metadata_obu_allocates_only_the_output() {
    for payload in [&[0x5a_u8; 16][..], &[0xb5; 300]] {
        let (mut out, mut size) = (ptr::null_mut(), 0);
        let allocs = allocs_during(|| {
            // SAFETY: `payload` is readable; the outputs are locals.
            unsafe { rs::metadata_obu(payload.as_ptr(), payload.len(), &mut out, &mut size, 4) };
        });
        let (mut t35, mut t35_size) = (ptr::null_mut(), 0);
        let t35_allocs = allocs_during(|| {
            // SAFETY: as above.
            unsafe {
                rs::metadata_obu_itu_t35(payload.as_ptr(), payload.len(), &mut t35, &mut t35_size)
            };
        });
        // SAFETY: both outputs came from bmalloc and are freed once.
        unsafe {
            bfree(out.cast());
            bfree(t35.cast());
        }
        assert!(size > payload.len() && t35_size == size);
        assert_eq!(
            (allocs, t35_allocs),
            (0, 0),
            "Rust heap allocations for a {}-byte payload",
            payload.len()
        );
    }
}
