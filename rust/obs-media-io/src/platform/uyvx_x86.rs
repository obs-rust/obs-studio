//! SSE2 equivalents of the C pack_shift/pack_val/pack_ch macros.
//! All stores are unaligned and retain the four-pixel block order.

use super::uyvx::Rows;
use core::arch::x86_64::{
    __m128i, _mm_add_epi16, _mm_and_si128, _mm_cvtsi128_si32, _mm_loadu_si128, _mm_packs_epi32,
    _mm_packus_epi16, _mm_set1_epi16, _mm_set1_epi32, _mm_shuffle_epi32, _mm_shufflelo_epi16,
    _mm_srli_epi16, _mm_srli_epi32, _mm_srli_si128,
};

/// Constructed only after the runtime SSE2 check.
pub(crate) struct Native(());

impl Native {
    #[inline]
    pub(super) fn detect() -> Option<Self> {
        std::arch::is_x86_feature_detected!("sse2").then_some(Self(()))
    }

    #[inline]
    pub(super) fn i420(&self, rows: Rows<'_>, u: &mut [u8], v: &mut [u8]) {
        rows.validate();
        assert!(rows.blocks <= u.len() / 2 && rows.blocks <= v.len() / 2);
        // SAFETY: this token guarantees SSE2; the assertions cover every block.
        unsafe { i420(rows, u.as_mut_ptr(), v.as_mut_ptr()) }
    }

    #[inline]
    pub(super) fn nv12(&self, rows: Rows<'_>, chroma: &mut [u8]) {
        rows.validate();
        assert!(rows.blocks <= chroma.len() / 4);
        // SAFETY: this token guarantees SSE2; the assertions cover every block.
        unsafe { nv12(rows, chroma.as_mut_ptr()) }
    }

    #[inline]
    pub(super) fn i444(&self, rows: Rows<'_>, u: &mut [u8], v: &mut [u8]) {
        rows.validate();
        assert!(rows.stride <= u.len() && rows.stride <= v.len());
        assert!(rows.blocks <= (u.len() - rows.stride) / 4);
        assert!(rows.blocks <= (v.len() - rows.stride) / 4);
        // SAFETY: this token guarantees SSE2; the assertions cover every block.
        unsafe { i444(rows, u.as_mut_ptr(), v.as_mut_ptr()) }
    }
}

#[inline]
#[target_feature(enable = "sse2")]
fn channel<const SHIFT: i32>(a: __m128i, b: __m128i) -> __m128i {
    let mask = _mm_set1_epi32(255);
    let a = _mm_and_si128(_mm_srli_epi32::<SHIFT>(a), mask);
    let b = _mm_and_si128(_mm_srli_epi32::<SHIFT>(b), mask);
    let packed = _mm_packs_epi32(a, b);
    _mm_packus_epi16(packed, packed)
}

#[inline]
#[target_feature(enable = "sse2")]
fn average(a: __m128i, b: __m128i) -> __m128i {
    let mask = _mm_set1_epi16(255);
    let sum = _mm_add_epi16(_mm_and_si128(a, mask), _mm_and_si128(b, mask));
    let pair = _mm_add_epi16(sum, _mm_shuffle_epi32::<0xb1>(sum));
    _mm_shuffle_epi32::<0xd8>(_mm_srli_epi16::<2>(pair))
}

#[inline]
#[target_feature(enable = "sse2")]
unsafe fn store_rows(values: __m128i, output: *mut u8, pixel: usize, stride: usize) {
    // SAFETY: each caller validates both four-byte output ranges before its loop.
    unsafe {
        output
            .add(pixel)
            .cast::<i32>()
            .write_unaligned(_mm_cvtsi128_si32(values));
        output
            .add(stride + pixel)
            .cast::<i32>()
            .write_unaligned(_mm_cvtsi128_si32(_mm_srli_si128::<4>(values)));
    }
}

#[inline]
#[target_feature(enable = "sse2")]
unsafe fn i420(rows: Rows<'_>, u: *mut u8, v: *mut u8) {
    // SAFETY: Native::i420 validated all input and output ranges, and the token
    // guarantees SSE2. Loading both rows before storing preserves the C order.
    unsafe {
        for i in 0..rows.blocks {
            let a = _mm_loadu_si128(rows.line1.as_ptr().add(i * 16).cast());
            let b = _mm_loadu_si128(rows.line2.as_ptr().add(i * 16).cast());
            store_rows(
                channel::<8>(a, b),
                rows.luma.as_mut_ptr(),
                i * 4,
                rows.stride,
            );
            let avg = _mm_shufflelo_epi16::<0xd8>(average(a, b));
            let packed = _mm_cvtsi128_si32(_mm_packus_epi16(avg, avg)) as u32;
            u.add(i * 2).cast::<u16>().write_unaligned(packed as u16);
            v.add(i * 2)
                .cast::<u16>()
                .write_unaligned((packed >> 16) as u16);
        }
    }
}

#[inline]
#[target_feature(enable = "sse2")]
unsafe fn nv12(rows: Rows<'_>, chroma: *mut u8) {
    // SAFETY: Native::nv12 validated all ranges and established SSE2 support.
    unsafe {
        for i in 0..rows.blocks {
            let a = _mm_loadu_si128(rows.line1.as_ptr().add(i * 16).cast());
            let b = _mm_loadu_si128(rows.line2.as_ptr().add(i * 16).cast());
            store_rows(
                channel::<8>(a, b),
                rows.luma.as_mut_ptr(),
                i * 4,
                rows.stride,
            );
            let avg = average(a, b);
            let packed = _mm_cvtsi128_si32(_mm_packus_epi16(avg, avg));
            chroma.add(i * 4).cast::<i32>().write_unaligned(packed);
        }
    }
}

#[inline]
#[target_feature(enable = "sse2")]
unsafe fn i444(rows: Rows<'_>, u: *mut u8, v: *mut u8) {
    // SAFETY: Native::i444 validated all ranges and established SSE2 support.
    unsafe {
        for i in 0..rows.blocks {
            let a = _mm_loadu_si128(rows.line1.as_ptr().add(i * 16).cast());
            let b = _mm_loadu_si128(rows.line2.as_ptr().add(i * 16).cast());
            store_rows(
                channel::<8>(a, b),
                rows.luma.as_mut_ptr(),
                i * 4,
                rows.stride,
            );
            store_rows(channel::<0>(a, b), u, i * 4, rows.stride);
            store_rows(channel::<16>(a, b), v, i * 4, rows.stride);
        }
    }
}
