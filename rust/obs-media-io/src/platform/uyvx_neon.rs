//! NEON byte-table extraction and exact widening 2x2 chroma averages.
//! Like the C reference, process four pixels before advancing the outputs.

use super::uyvx::Rows;
use core::arch::aarch64::{
    uint8x8_t, uint8x16_t, vaddl_u8, vget_lane_u16, vget_lane_u32, vget_low_u8, vld1q_u8,
    vpaddq_u16, vqtbl1q_u8, vreinterpret_u16_u8, vreinterpret_u32_u8, vshrn_n_u16, vzip1_u8,
};

/// Constructed only after the runtime NEON check.
pub(crate) struct Native(());

impl Native {
    #[inline]
    pub(super) fn detect() -> Option<Self> {
        std::arch::is_aarch64_feature_detected!("neon").then_some(Self(()))
    }

    #[inline]
    pub(super) fn i420(&self, rows: Rows<'_>, u: &mut [u8], v: &mut [u8]) {
        rows.validate();
        assert!(rows.blocks <= u.len() / 2 && rows.blocks <= v.len() / 2);
        // SAFETY: this token guarantees NEON; the assertions cover every block.
        unsafe { i420(rows, u.as_mut_ptr(), v.as_mut_ptr()) }
    }

    #[inline]
    pub(super) fn nv12(&self, rows: Rows<'_>, chroma: &mut [u8]) {
        rows.validate();
        assert!(rows.blocks <= chroma.len() / 4);
        // SAFETY: this token guarantees NEON; the assertions cover every block.
        unsafe { nv12(rows, chroma.as_mut_ptr()) }
    }

    #[inline]
    pub(super) fn i444(&self, rows: Rows<'_>, u: &mut [u8], v: &mut [u8]) {
        rows.validate();
        assert!(rows.stride <= u.len() && rows.stride <= v.len());
        assert!(rows.blocks <= (u.len() - rows.stride) / 4);
        assert!(rows.blocks <= (v.len() - rows.stride) / 4);
        // SAFETY: this token guarantees NEON; the assertions cover every block.
        unsafe { i444(rows, u.as_mut_ptr(), v.as_mut_ptr()) }
    }
}

#[inline]
#[target_feature(enable = "neon")]
fn channel<const C: u8>(row: uint8x16_t) -> uint8x8_t {
    let indices = [
        C,
        C + 4,
        C + 8,
        C + 12,
        255,
        255,
        255,
        255,
        255,
        255,
        255,
        255,
        255,
        255,
        255,
        255,
    ];
    // SAFETY: the local array has exactly the sixteen bytes this load reads.
    let indices = unsafe { vld1q_u8(indices.as_ptr()) };
    vget_low_u8(vqtbl1q_u8(row, indices))
}

#[inline]
#[target_feature(enable = "neon")]
fn average(a: uint8x8_t, b: uint8x8_t) -> uint8x8_t {
    let sum = vaddl_u8(a, b);
    vshrn_n_u16::<2>(vpaddq_u16(sum, sum))
}

#[inline]
#[target_feature(enable = "neon")]
unsafe fn store_rows(a: uint8x8_t, b: uint8x8_t, out: *mut u8, pixel: usize, stride: usize) {
    // SAFETY: callers validate both four-byte stores before entering the loop.
    unsafe {
        out.add(pixel)
            .cast::<u32>()
            .write_unaligned(vget_lane_u32::<0>(vreinterpret_u32_u8(a)));
        out.add(stride + pixel)
            .cast::<u32>()
            .write_unaligned(vget_lane_u32::<0>(vreinterpret_u32_u8(b)));
    }
}

#[inline]
#[target_feature(enable = "neon")]
unsafe fn i420(rows: Rows<'_>, u: *mut u8, v: *mut u8) {
    // SAFETY: Native::i420 validated all ranges and established NEON support.
    unsafe {
        for i in 0..rows.blocks {
            let a = vld1q_u8(rows.line1.as_ptr().add(i * 16));
            let b = vld1q_u8(rows.line2.as_ptr().add(i * 16));
            store_rows(
                channel::<1>(a),
                channel::<1>(b),
                rows.luma.as_mut_ptr(),
                i * 4,
                rows.stride,
            );
            let us = average(channel::<0>(a), channel::<0>(b));
            let vs = average(channel::<2>(a), channel::<2>(b));
            u.add(i * 2)
                .cast::<u16>()
                .write_unaligned(vget_lane_u16::<0>(vreinterpret_u16_u8(us)));
            v.add(i * 2)
                .cast::<u16>()
                .write_unaligned(vget_lane_u16::<0>(vreinterpret_u16_u8(vs)));
        }
    }
}

#[inline]
#[target_feature(enable = "neon")]
unsafe fn nv12(rows: Rows<'_>, chroma: *mut u8) {
    // SAFETY: Native::nv12 validated all ranges and established NEON support.
    unsafe {
        for i in 0..rows.blocks {
            let a = vld1q_u8(rows.line1.as_ptr().add(i * 16));
            let b = vld1q_u8(rows.line2.as_ptr().add(i * 16));
            store_rows(
                channel::<1>(a),
                channel::<1>(b),
                rows.luma.as_mut_ptr(),
                i * 4,
                rows.stride,
            );
            let us = average(channel::<0>(a), channel::<0>(b));
            let vs = average(channel::<2>(a), channel::<2>(b));
            let packed = vzip1_u8(us, vs);
            chroma
                .add(i * 4)
                .cast::<u32>()
                .write_unaligned(vget_lane_u32::<0>(vreinterpret_u32_u8(packed)));
        }
    }
}

#[inline]
#[target_feature(enable = "neon")]
unsafe fn i444(rows: Rows<'_>, u: *mut u8, v: *mut u8) {
    // SAFETY: Native::i444 validated all ranges and established NEON support.
    unsafe {
        for i in 0..rows.blocks {
            let a = vld1q_u8(rows.line1.as_ptr().add(i * 16));
            let b = vld1q_u8(rows.line2.as_ptr().add(i * 16));
            store_rows(
                channel::<1>(a),
                channel::<1>(b),
                rows.luma.as_mut_ptr(),
                i * 4,
                rows.stride,
            );
            store_rows(channel::<0>(a), channel::<0>(b), u, i * 4, rows.stride);
            store_rows(channel::<2>(a), channel::<2>(b), v, i * 4, rows.stride);
        }
    }
}
