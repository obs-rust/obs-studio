//! Tier 3: the Rust C ABI shims and safe core behave exactly like the
//! original C, compiled as an oracle.
//!
//! No intentional differences from C. The C's SSE2 is integer only, so
//! parity is exact byte equality, with no ULP allowance. Every call runs
//! four ways on identical buffers: the C with native SSE2 on x86
//! (`oracle_*`), the C with the plain-C `__m128i` stand-ins in
//! `oracle/sse-shim` (`oracle_scalar_*`, what non-x86 builds use for both),
//! the Rust shim and the Rust safe core on byte slices.
//!
//! Every buffer is 16-byte aligned and sized to the bytes the call touches,
//! plus `SLACK` canary bytes. Outputs start as canary and are compared whole,
//! so a write outside the expected area shows up as a mismatch, and the safe
//! core panics if a size is too small.
//!
//! Excluded inputs, where the C reads or writes misaligned or out of bounds
//! (UB), or never terminates:
//! - UYVX functions: `in_linesize` not a multiple of 16, or an input not
//!   16-byte aligned (`_mm_load_si128` is an aligned load);
//!   `out_linesize[0]` not a multiple of 4 (luma is stored as `uint32_t`);
//!   an I420 `out_linesize[1]` that is odd or an NV12 one that is not a
//!   multiple of 4 (chroma is stored as `uint16_t`/`uint32_t`).
//! - `decompress_*`: `out_linesize` not a multiple of 4 (pixels are stored
//!   as `uint32_t`); an odd NV12 `in_linesize[1]` (chroma is read as
//!   `uint16_t`); a `decompress_422` `in_linesize` not a multiple of 4.
//! - Widths or row ranges near `u32::MAX`, where the C's `x += 4` or
//!   `y += 2` wraps and the loop never ends, and offsets whose `u32`
//!   products wrap beyond a test-sized buffer. The Rust core wraps the same
//!   way; [`decompress_422_wrapping_row_offset`] covers the one wrap a small
//!   buffer can reach.
//! - Input and output planes that overlap each other.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use obs_c_oracle::format_conversion as c;
use obs_media_io::ffi::format_conversion as rs;
use obs_media_io::format_conversion as safe;
use proptest::prelude::*;

const CANARY: u8 = 0xA5;
const SLACK: usize = 16;

/// A 16-byte-aligned buffer of `len` bytes plus `SLACK`.
#[derive(Clone)]
struct Buf {
    storage: Vec<u8>,
    start: usize,
    len: usize,
}

impl Buf {
    fn canary(len: usize) -> Self {
        let storage = vec![CANARY; len + SLACK + 15];
        let start = storage.as_ptr().align_offset(16);
        Buf {
            storage,
            start,
            len,
        }
    }

    /// Pseudo-random contents from `seed` (splitmix64).
    fn random(len: usize, seed: u64) -> Self {
        let mut buf = Self::canary(len);
        let mut state = seed;
        for b in buf.bytes_mut() {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            *b = (z ^ (z >> 31)) as u8;
        }
        buf
    }

    fn bytes(&self) -> &[u8] {
        &self.storage[self.start..self.start + self.len + SLACK]
    }

    fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.storage[self.start..self.start + self.len + SLACK]
    }

    fn ptr(&self) -> *const u8 {
        self.bytes().as_ptr()
    }

    fn mut_ptr(&mut self) -> *mut u8 {
        self.bytes_mut().as_mut_ptr()
    }
}

/// The last `y` of `for (y = start; y < end; y += step)`, if any.
fn last_row(start: u32, end: u32, step: u32) -> Option<usize> {
    (start < end).then(|| (start + (end - 1 - start) / step * step) as usize)
}

/// The four implementations, in comparison order. The safe core runs before
/// the shim, so a panic in it fails the test instead of aborting at the
/// `extern "C"` boundary.
#[derive(Debug, Clone, Copy)]
enum Impl {
    C,
    CScalar,
    Safe,
    Shim,
}

const IMPLS: [Impl; 4] = [Impl::C, Impl::CScalar, Impl::Safe, Impl::Shim];

/// Runs `call` once per implementation on fresh copies of `outputs` and
/// asserts every result equals the native C one.
fn assert_parity(outputs: &[Buf], mut call: impl FnMut(Impl, &mut [Buf])) {
    let mut results = Vec::new();
    for imp in IMPLS {
        let mut out = outputs.to_vec();
        call(imp, &mut out);
        let bytes: Vec<Vec<u8>> = out.iter().map(|b| b.bytes().to_vec()).collect();
        results.push((imp, bytes));
    }
    let (_, expected) = &results[0];
    for (imp, bytes) in &results[1..] {
        for (plane, (got, want)) in bytes.iter().zip(expected).enumerate() {
            assert_eq!(got, want, "{imp:?} differs from C in plane {plane}");
        }
    }
}

/// The parameters of one UYVX call.
#[derive(Debug, Clone, Copy)]
struct Uyvx {
    in_linesize: u32,
    out_linesize: [u32; 2],
    start_y: u32,
    end_y: u32,
    seed: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum UyvxKind {
    I420,
    Nv12,
    I444,
}

impl Uyvx {
    fn width(&self) -> usize {
        self.in_linesize.min(self.out_linesize[0]) as usize
    }

    /// Bytes of input and of each output plane the call touches.
    fn sizes(&self, kind: UyvxKind) -> (usize, [usize; 3]) {
        let width = self.width();
        let Some(y) = last_row(self.start_y, self.end_y, 2).filter(|_| width > 0) else {
            return (0, [0; 3]);
        };
        let [ls0, ls1] = self.out_linesize.map(|l| l as usize);
        let input = (y + 1) * self.in_linesize as usize + width * 4;
        let lum = (y + 1) * ls0 + width;
        let chroma = match kind {
            UyvxKind::I420 => (y / 2) * ls1 + width / 2,
            UyvxKind::Nv12 => (y / 2) * ls1 + width,
            UyvxKind::I444 => lum,
        };
        (input, [lum, chroma, chroma])
    }

    fn check(&self, kind: UyvxKind) {
        let (input_len, plane_lens) = self.sizes(kind);
        let planes = if kind == UyvxKind::Nv12 { 2 } else { 3 };
        let input = Buf::random(input_len, self.seed);
        let outputs: Vec<Buf> = plane_lens[..planes]
            .iter()
            .map(|&l| Buf::canary(l))
            .collect();
        // The C reads out_linesize[2] nowhere; pass a value no one uses.
        let linesizes = [self.out_linesize[0], self.out_linesize[1], 0xDEAD];

        assert_parity(&outputs, |imp, out| {
            let mut ptrs: Vec<*mut u8> = out.iter_mut().map(Buf::mut_ptr).collect();
            let (ls, start_y, end_y) = (self.in_linesize, self.start_y, self.end_y);
            match imp {
                Impl::Safe => {
                    let mut slices = out.iter_mut().map(Buf::bytes_mut);
                    let mut next = || slices.next().unwrap();
                    match kind {
                        UyvxKind::I420 => safe::compress_uyvx_to_i420(
                            input.bytes(),
                            ls,
                            start_y,
                            end_y,
                            [next(), next(), next()],
                            &linesizes,
                        ),
                        UyvxKind::Nv12 => safe::compress_uyvx_to_nv12(
                            input.bytes(),
                            ls,
                            start_y,
                            end_y,
                            [next(), next()],
                            &linesizes,
                        ),
                        UyvxKind::I444 => safe::convert_uyvx_to_i444(
                            input.bytes(),
                            ls,
                            start_y,
                            end_y,
                            [next(), next(), next()],
                            linesizes[0],
                        ),
                    }
                }
                _ => {
                    let f = match (imp, kind) {
                        (Impl::C, UyvxKind::I420) => c::oracle_compress_uyvx_to_i420,
                        (Impl::C, UyvxKind::Nv12) => c::oracle_compress_uyvx_to_nv12,
                        (Impl::C, UyvxKind::I444) => c::oracle_convert_uyvx_to_i444,
                        (Impl::CScalar, UyvxKind::I420) => c::oracle_scalar_compress_uyvx_to_i420,
                        (Impl::CScalar, UyvxKind::Nv12) => c::oracle_scalar_compress_uyvx_to_nv12,
                        (Impl::CScalar, UyvxKind::I444) => c::oracle_scalar_convert_uyvx_to_i444,
                        (Impl::Shim, UyvxKind::I420) => rs::compress_uyvx_to_i420,
                        (Impl::Shim, UyvxKind::Nv12) => rs::compress_uyvx_to_nv12,
                        (Impl::Shim, UyvxKind::I444) => rs::convert_uyvx_to_i444,
                        (Impl::Safe, _) => unreachable!(),
                    };
                    // SAFETY: the input and every plane are aligned and sized
                    // to cover the bytes this call touches, and the plane and
                    // linesize arrays have 3 entries.
                    unsafe {
                        f(
                            input.ptr(),
                            ls,
                            start_y,
                            end_y,
                            ptrs.as_mut_ptr().cast_const(),
                            linesizes.as_ptr(),
                        );
                    }
                }
            }
        });
    }
}

fn uyvx_strategy(chroma_align: u32) -> impl Strategy<Value = Uyvx> {
    (
        0u32..=6,
        0u32..=10,
        0u32..=12,
        0u32..=7,
        0u32..=10,
        any::<u64>(),
    )
        .prop_map(move |(ls, l0, l1, start_y, end_y, seed)| Uyvx {
            in_linesize: 16 * ls,
            out_linesize: [4 * l0, chroma_align * l1],
            start_y,
            end_y,
            seed,
        })
}

/// The parameters of one `decompress_420`/`decompress_nv12` call.
#[derive(Debug, Clone, Copy)]
struct Planar {
    in_linesize: [u32; 3],
    out_linesize: u32,
    start_y: u32,
    end_y: u32,
    seed: u64,
}

impl Planar {
    fn check_420(&self) {
        let [l0, l1, l2] = self.in_linesize.map(|l| l as usize);
        let width_d2 = l0 / 2;
        let rows = last_row(self.start_y / 2, self.end_y / 2, 1).filter(|_| width_d2 > 0);
        let (sizes, out_len) = match rows {
            Some(y) => (
                [
                    (2 * y + 1) * l0 + 2 * width_d2,
                    y * l1 + width_d2,
                    y * l2 + width_d2,
                ],
                (2 * y + 1) * self.out_linesize as usize + 8 * width_d2,
            ),
            None => ([0; 3], 0),
        };
        let planes: Vec<Buf> = (0u64..)
            .zip(sizes)
            .map(|(i, len)| Buf::random(len, self.seed ^ i))
            .collect();
        let ptrs: Vec<*const u8> = planes.iter().map(Buf::ptr).collect();
        let (start_y, end_y, out_ls) = (self.start_y, self.end_y, self.out_linesize);

        assert_parity(&[Buf::canary(out_len)], |imp, out| {
            let f = match imp {
                Impl::C => c::oracle_decompress_420,
                Impl::CScalar => c::oracle_scalar_decompress_420,
                Impl::Shim => rs::decompress_420,
                Impl::Safe => {
                    let input = [planes[0].bytes(), planes[1].bytes(), planes[2].bytes()];
                    let output = out[0].bytes_mut();
                    safe::decompress_420(input, &self.in_linesize, start_y, end_y, output, out_ls);
                    return;
                }
            };
            // SAFETY: every plane and the output are sized to cover the
            // bytes this call touches; both arrays have 3 entries.
            unsafe {
                f(
                    ptrs.as_ptr(),
                    self.in_linesize.as_ptr(),
                    start_y,
                    end_y,
                    out[0].mut_ptr(),
                    out_ls,
                );
            }
        });
    }

    fn check_nv12(&self) {
        let [l0, l1, _] = self.in_linesize.map(|l| l as usize);
        let width_d2 = self.in_linesize[0].min(self.out_linesize) as usize / 2;
        let rows = last_row(self.start_y / 2, self.end_y / 2, 1).filter(|_| width_d2 > 0);
        let (sizes, out_len) = match rows {
            Some(y) => (
                [(2 * y + 1) * l0 + 2 * width_d2, y * l1 + 2 * width_d2],
                (2 * y + 1) * self.out_linesize as usize + 8 * width_d2,
            ),
            None => ([0; 2], 0),
        };
        let planes: Vec<Buf> = (0u64..)
            .zip(sizes)
            .map(|(i, len)| Buf::random(len, self.seed ^ i))
            .collect();
        let ptrs: Vec<*const u8> = planes.iter().map(Buf::ptr).collect();
        let (start_y, end_y, out_ls) = (self.start_y, self.end_y, self.out_linesize);

        assert_parity(&[Buf::canary(out_len)], |imp, out| {
            let f = match imp {
                Impl::C => c::oracle_decompress_nv12,
                Impl::CScalar => c::oracle_scalar_decompress_nv12,
                Impl::Shim => rs::decompress_nv12,
                Impl::Safe => {
                    let input = [planes[0].bytes(), planes[1].bytes()];
                    let output = out[0].bytes_mut();
                    safe::decompress_nv12(input, &self.in_linesize, start_y, end_y, output, out_ls);
                    return;
                }
            };
            // SAFETY: both planes and the output are sized to cover the
            // bytes this call touches; both arrays have at least 2 entries.
            unsafe {
                f(
                    ptrs.as_ptr(),
                    self.in_linesize.as_ptr(),
                    start_y,
                    end_y,
                    out[0].mut_ptr(),
                    out_ls,
                );
            }
        });
    }
}

fn planar_strategy(chroma_align: u32) -> impl Strategy<Value = Planar> {
    (
        0u32..=20,
        0u32..=12,
        0u32..=12,
        0u32..=20,
        0u32..=8,
        0u32..=10,
        any::<u64>(),
    )
        .prop_map(move |(l0, l1, l2, out, start_y, end_y, seed)| Planar {
            in_linesize: [l0, chroma_align * l1, l2],
            out_linesize: 4 * out,
            start_y,
            end_y,
            seed,
        })
}

/// The parameters of one `decompress_422` call.
#[derive(Debug, Clone, Copy)]
struct Packed {
    in_linesize: u32,
    out_linesize: u32,
    start_y: u32,
    end_y: u32,
    leading_lum: bool,
    seed: u64,
}

impl Packed {
    fn check(&self) {
        let width_d2 = self.in_linesize.min(self.out_linesize) as usize / 4;
        // Only the last row's offsets matter: rows are visited in order.
        let rows = last_row(self.start_y, self.end_y, 1).filter(|_| width_d2 > 0);
        let (in_len, out_len) = match rows {
            Some(y) => (
                (y as u32).wrapping_mul(self.in_linesize) as usize + 4 * width_d2,
                y * self.out_linesize as usize + 8 * width_d2,
            ),
            None => (0, 0),
        };
        let input = Buf::random(in_len, self.seed);
        assert_parity(&[Buf::canary(out_len)], |imp, out| {
            self.call(imp, &input, &mut out[0]);
        });
    }

    /// Runs `imp` on `input` and `output`, which must cover the bytes the
    /// call touches.
    fn call(&self, imp: Impl, input: &Buf, output: &mut Buf) {
        let (ls, start_y, end_y) = (self.in_linesize, self.start_y, self.end_y);
        let (out_ls, leading) = (self.out_linesize, self.leading_lum);
        let f = match imp {
            Impl::C => c::oracle_decompress_422,
            Impl::CScalar => c::oracle_scalar_decompress_422,
            Impl::Shim => rs::decompress_422,
            Impl::Safe => {
                safe::decompress_422(
                    input.bytes(),
                    ls,
                    start_y,
                    end_y,
                    output.bytes_mut(),
                    out_ls,
                    leading,
                );
                return;
            }
        };
        // SAFETY: the input and output are sized to cover the bytes this
        // call touches.
        unsafe {
            f(
                input.ptr(),
                ls,
                start_y,
                end_y,
                output.mut_ptr(),
                out_ls,
                leading,
            )
        };
    }
}

/// One packed 4:2:2 row of 4 pixels (`in_linesize` 8) converts to one row of
/// 4 32-bit pixels (`out_linesize` 16), without the next row, and leaves the
/// canary after the output, in every implementation (#3031). The inputs and expected values
/// of `test_decompress_422_one_row` in `test/cmocka/test_format_conversion.c`.
#[test]
fn decompress_422_one_row() {
    // The row, then the next row's first 2 dwords, which must not be read.
    let mut input = Buf::canary(16);
    let dwords = [0x4433_2211u32, 0x8877_6655, 0xCCBB_AA99, 0x00FF_EEDD];
    for (i, dw) in dwords.into_iter().enumerate() {
        input.bytes_mut()[4 * i..4 * i + 4].copy_from_slice(&dw.to_ne_bytes());
    }
    for (leading_lum, dup) in [
        (false, [0x4433_4411u32, 0x8877_8855]),
        (true, [0x4433_2233, 0x8877_6677]),
    ] {
        let mut expected = Buf::canary(16);
        for (i, dw) in [0x4433_2211u32, dup[0], 0x8877_6655, dup[1]]
            .into_iter()
            .enumerate()
        {
            expected.bytes_mut()[4 * i..4 * i + 4].copy_from_slice(&dw.to_ne_bytes());
        }
        let p = Packed {
            in_linesize: 8,
            out_linesize: 16,
            start_y: 0,
            end_y: 1,
            leading_lum,
            seed: 0,
        };
        for imp in IMPLS {
            let mut output = Buf::canary(16);
            p.call(imp, &input, &mut output);
            assert_eq!(
                output.bytes(),
                expected.bytes(),
                "{imp:?}, leading_lum {leading_lum}"
            );
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]

    #[test]
    fn compress_uyvx_to_i420_matches_c(p in uyvx_strategy(2)) {
        p.check(UyvxKind::I420);
    }

    #[test]
    fn compress_uyvx_to_nv12_matches_c(p in uyvx_strategy(4)) {
        p.check(UyvxKind::Nv12);
    }

    #[test]
    fn convert_uyvx_to_i444_matches_c(p in uyvx_strategy(1)) {
        p.check(UyvxKind::I444);
    }

    #[test]
    fn decompress_420_matches_c(p in planar_strategy(1)) {
        p.check_420();
    }

    #[test]
    fn decompress_nv12_matches_c(p in planar_strategy(2)) {
        p.check_nv12();
    }

    #[test]
    fn decompress_422_matches_c(
        (ls, out, start_y, end_y, leading_lum, seed) in
            (0u32..=8, 0u32..=16, 0u32..=6, 0u32..=8, any::<bool>(), any::<u64>())
    ) {
        Packed {
            in_linesize: 4 * ls,
            out_linesize: 4 * out,
            start_y,
            end_y,
            leading_lum,
            seed,
        }
        .check();
    }
}

/// Row 2 of a 2^31-byte `in_linesize` starts at `u32` offset 0 in the C.
#[test]
fn decompress_422_wrapping_row_offset() {
    for leading_lum in [false, true] {
        Packed {
            in_linesize: 1 << 31,
            out_linesize: 8,
            start_y: 2,
            end_y: 3,
            leading_lum,
            seed: 0x0123_4567_89AB_CDEF,
        }
        .check();
    }
}

/// The inputs of `test/cmocka/test_format_conversion.c`, sized as there.
#[test]
fn cmocka_inputs() {
    for (kind, out_linesize, start_y, end_y) in [
        (UyvxKind::I420, [8, 4], 0, 4),
        (UyvxKind::I420, [8, 4], 2, 4),
        (UyvxKind::I420, [4, 2], 0, 4),
        (UyvxKind::Nv12, [8, 8], 0, 4),
        (UyvxKind::Nv12, [8, 16], 2, 4),
        (UyvxKind::I444, [8, 8], 0, 4),
        (UyvxKind::I444, [8, 1], 2, 4),
    ] {
        Uyvx {
            in_linesize: 32,
            out_linesize,
            start_y,
            end_y,
            seed: 1,
        }
        .check(kind);
    }
    for (in_linesize, out_linesize, start_y, end_y) in [
        ([8, 6, 8], 40, 0, 4),
        ([8, 4, 4], 32, 1, 3),
        ([8, 4, 4], 32, 2, 3),
        ([7, 4, 4], 32, 0, 2),
    ] {
        let p = Planar {
            in_linesize,
            out_linesize,
            start_y,
            end_y,
            seed: 2,
        };
        p.check_420();
    }
    for (in_linesize, out_linesize, start_y, end_y) in
        [([8, 12, 0], 40, 0, 4), ([8, 8, 0], 32, 3, 4)]
    {
        Planar {
            in_linesize,
            out_linesize,
            start_y,
            end_y,
            seed: 3,
        }
        .check_nv12();
    }
    for (in_linesize, out_linesize, end_y) in [(4, 8, 1), (8, 16, 2)] {
        for leading_lum in [false, true] {
            Packed {
                in_linesize,
                out_linesize,
                start_y: 0,
                end_y,
                leading_lum,
                seed: 4,
            }
            .check();
        }
    }
}
