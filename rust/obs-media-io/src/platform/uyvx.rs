//! Direct-output UYVX row kernels and their runtime selection (#3004).
//!
//! Keep the C's four-pixel block order, including its overlapping output rows.
//! SIMD stays within one block and stores into the caller's borrowed rows.

use super::uyvx_native::Native;

const UYVX_U: usize = 0;
const UYVX_Y: usize = 1;
const UYVX_V: usize = 2;
const BLOCK_BYTES: usize = 16;

/// One borrowed pair of input/luma rows; the second luma row may overlap the first.
pub(crate) struct Rows<'a> {
    pub(crate) line1: &'a [u8],
    pub(crate) line2: &'a [u8],
    pub(crate) luma: &'a mut [u8],
    pub(crate) stride: usize,
    pub(crate) blocks: usize,
}

impl Rows<'_> {
    /// Establish the bounds that each native block's loads and stores use.
    #[inline(always)]
    pub(super) fn validate(&self) {
        assert!(self.blocks <= self.line1.len() / BLOCK_BYTES);
        assert!(self.blocks <= self.line2.len() / BLOCK_BYTES);
        assert!(self.stride <= self.luma.len());
        assert!(self.blocks <= (self.luma.len() - self.stride) / 4);
        assert!(self.blocks <= self.luma.len() / 4);
    }
}

/// Selected once per conversion, outside both the row and pixel loops.
pub(crate) enum Kernel {
    Scalar,
    Native(Native),
}

impl Kernel {
    #[inline(always)]
    pub(crate) fn detect() -> Self {
        #[cfg(test)]
        if FORCE.with(core::cell::Cell::get) == Some(false) {
            return Self::Scalar;
        }
        Native::detect().map_or(Self::Scalar, Self::Native)
    }

    #[inline(always)]
    pub(crate) fn i420(&self, rows: Rows<'_>, u: &mut [u8], v: &mut [u8]) {
        match self {
            Self::Scalar => scalar_i420(rows, u, v),
            Self::Native(native) => native.i420(rows, u, v),
        }
    }

    #[inline(always)]
    pub(crate) fn nv12(&self, rows: Rows<'_>, chroma: &mut [u8]) {
        match self {
            Self::Scalar => scalar_nv12(rows, chroma),
            Self::Native(native) => native.nv12(rows, chroma),
        }
    }

    #[inline(always)]
    pub(crate) fn i444(&self, rows: Rows<'_>, u: &mut [u8], v: &mut [u8]) {
        match self {
            Self::Scalar => scalar_i444(rows, u, v),
            Self::Native(native) => native.i444(rows, u, v),
        }
    }
}

#[cfg(test)]
std::thread_local! {
    static FORCE: core::cell::Cell<Option<bool>> = const { core::cell::Cell::new(None) };
}

/// Force this thread's selection, restoring it even when an assertion panics.
#[cfg(test)]
pub(crate) fn with_native_for_test(native: bool, run: impl FnOnce()) {
    if native && Native::detect().is_none() {
        return; // This architecture has only the scalar fallback.
    }
    struct Reset(Option<bool>);
    impl Drop for Reset {
        fn drop(&mut self) {
            FORCE.with(|force| force.set(self.0));
        }
    }
    let _reset = Reset(FORCE.with(|force| force.replace(Some(native))));
    assert_eq!(matches!(Kernel::detect(), Kernel::Native(_)), native);
    println!(
        "UYVX row kernel: {} on {}",
        if native { "native" } else { "scalar" },
        std::env::consts::ARCH
    );
    run();
}

/// The four bytes of channel `c` in the four UYVX pixels at `at` in `row`.
fn channel(row: &[u8], at: usize, c: usize) -> [u8; 4] {
    [
        row[at + c],
        row[at + 4 + c],
        row[at + 8 + c],
        row[at + 12 + c],
    ]
}

/// Channel `c` averaged over pixels 0-1 and 2-3 of the two rows at `at`,
/// `(a + b + c + d) >> 2`.
fn average(line1: &[u8], line2: &[u8], at: usize, c: usize) -> [u8; 2] {
    let avg = |p: usize| {
        let sum = u16::from(line1[at + p + c])
            + u16::from(line1[at + p + 4 + c])
            + u16::from(line2[at + p + c])
            + u16::from(line2[at + p + 4 + c]);
        (sum >> 2) as u8
    };
    [avg(0), avg(8)]
}

fn scalar_i420(rows: Rows<'_>, u: &mut [u8], v: &mut [u8]) {
    let Rows {
        line1,
        line2,
        luma,
        stride,
        blocks,
    } = rows;
    for i in 0..blocks {
        let (pixel, block) = (i * 4, i * BLOCK_BYTES);
        luma[pixel..pixel + 4].copy_from_slice(&channel(line1, block, UYVX_Y));
        luma[stride + pixel..stride + pixel + 4].copy_from_slice(&channel(line2, block, UYVX_Y));
        u[i * 2..i * 2 + 2].copy_from_slice(&average(line1, line2, block, UYVX_U));
        v[i * 2..i * 2 + 2].copy_from_slice(&average(line1, line2, block, UYVX_V));
    }
}

fn scalar_nv12(rows: Rows<'_>, chroma: &mut [u8]) {
    let Rows {
        line1,
        line2,
        luma,
        stride,
        blocks,
    } = rows;
    for i in 0..blocks {
        let (pixel, block) = (i * 4, i * BLOCK_BYTES);
        luma[pixel..pixel + 4].copy_from_slice(&channel(line1, block, UYVX_Y));
        luma[stride + pixel..stride + pixel + 4].copy_from_slice(&channel(line2, block, UYVX_Y));
        let [u01, u23] = average(line1, line2, block, UYVX_U);
        let [v01, v23] = average(line1, line2, block, UYVX_V);
        chroma[pixel..pixel + 4].copy_from_slice(&[u01, v01, u23, v23]);
    }
}

fn scalar_i444(rows: Rows<'_>, u: &mut [u8], v: &mut [u8]) {
    let Rows {
        line1,
        line2,
        luma,
        stride,
        blocks,
    } = rows;
    for i in 0..blocks {
        let (pixel, block) = (i * 4, i * BLOCK_BYTES);
        for (output, c) in [(&mut *luma, UYVX_Y), (&mut *u, UYVX_U), (&mut *v, UYVX_V)] {
            output[pixel..pixel + 4].copy_from_slice(&channel(line1, block, c));
            output[stride + pixel..stride + pixel + 4].copy_from_slice(&channel(line2, block, c));
        }
    }
}
