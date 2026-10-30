//! #3003: `libobs/media-io/format-conversion.c.h` shims against the C
//! oracle, the `format` category of #3009. Every case converts one 1080p
//! frame. `format/<case>/1080p` pairs the Rust with the C as it ships
//! (SSE2 on x86); `format/<case>_c_scalar/1080p` pairs it with the same C
//! built on the plain-C `__m128i` stand-ins, which is what non-x86 builds
//! run for both. Before timing, each case checks that both sides write the
//! same bytes.

use std::hint::black_box;

use criterion::Criterion;
use obs_c_oracle::format_conversion as c;
use obs_media_io::ffi::format_conversion as rs;

const WIDTH: usize = 1920;
const HEIGHT: usize = 1080;

type Compress = unsafe extern "C" fn(*const u8, u32, u32, u32, *const *mut u8, *const u32);
type Decompress = unsafe extern "C" fn(*const *const u8, *const u32, u32, u32, *mut u8, u32);
type Decompress422 = unsafe extern "C" fn(*const u8, u32, u32, u32, *mut u8, u32, bool);

/// `len` deterministic bytes (an LCG), the same on every run.
fn bytes(len: usize, seed: u32) -> Vec<u8> {
    let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
    (0..len)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 24) as u8
        })
        .collect()
}

/// One UYVX frame packed into planes of the given linesizes and heights.
fn compress(
    criterion: &mut Criterion,
    case: &str,
    plane_rows: [(usize, usize); 3],
    linesizes: [u32; 2],
    fns: [Compress; 3],
) {
    let input = bytes(WIDTH * 4 * HEIGHT, 1);
    let planes = || plane_rows.map(|(linesize, rows)| vec![0u8; linesize * rows]);
    let run = |f: Compress, out: &mut [Vec<u8>; 3]| {
        let ptrs = [
            out[0].as_mut_ptr(),
            out[1].as_mut_ptr(),
            out[2].as_mut_ptr(),
        ];
        // SAFETY: a whole frame: `input` holds HEIGHT rows of WIDTH UYVX
        // pixels and each plane holds its rows at its linesize.
        unsafe {
            f(
                black_box(input.as_ptr()),
                (WIDTH * 4) as u32,
                0,
                HEIGHT as u32,
                ptrs.as_ptr(),
                linesizes.as_ptr(),
            )
        }
    };
    let [rust, shipped, scalar] = fns;
    let (mut r, mut k, mut s) = (planes(), planes(), planes());
    run(rust, &mut r);
    run(shipped, &mut k);
    run(scalar, &mut s);
    assert_eq!(r, k, "benchmark fixture differs: format/{case}");
    assert_eq!(r, s, "benchmark fixture differs: format/{case}_c_scalar");
    super::measure_pair(
        criterion,
        &format!("format/{case}/1080p"),
        || run(rust, &mut r),
        || run(shipped, &mut k),
    );
    super::measure_pair(
        criterion,
        &format!("format/{case}_c_scalar/1080p"),
        || run(rust, &mut r),
        || run(scalar, &mut s),
    );
}

/// One planar frame unpacked into UYVX.
fn decompress(
    criterion: &mut Criterion,
    case: &str,
    plane_rows: [(usize, usize); 3],
    fns: [Decompress; 3],
) {
    let inputs: Vec<Vec<u8>> = (0u32..)
        .zip(plane_rows)
        .map(|(seed, (linesize, rows))| bytes(linesize * rows, seed + 2))
        .collect();
    let ptrs: Vec<*const u8> = inputs.iter().map(|plane| plane.as_ptr()).collect();
    let linesizes = plane_rows.map(|(linesize, _)| linesize as u32);
    let run = |f: Decompress, out: &mut Vec<u8>| {
        // SAFETY: a whole frame: each input plane holds its rows at its
        // linesize and `out` holds HEIGHT rows of WIDTH UYVX pixels.
        unsafe {
            f(
                black_box(ptrs.as_ptr()),
                linesizes.as_ptr(),
                0,
                HEIGHT as u32,
                out.as_mut_ptr(),
                (WIDTH * 4) as u32,
            )
        }
    };
    let [rust, shipped, scalar] = fns;
    let frame = || vec![0u8; WIDTH * 4 * HEIGHT];
    let (mut r, mut k, mut s) = (frame(), frame(), frame());
    run(rust, &mut r);
    run(shipped, &mut k);
    run(scalar, &mut s);
    assert!(r == k, "benchmark fixture differs: format/{case}");
    assert!(r == s, "benchmark fixture differs: format/{case}_c_scalar");
    super::measure_pair(
        criterion,
        &format!("format/{case}/1080p"),
        || run(rust, &mut r),
        || run(shipped, &mut k),
    );
    super::measure_pair(
        criterion,
        &format!("format/{case}_c_scalar/1080p"),
        || run(rust, &mut r),
        || run(scalar, &mut s),
    );
}

/// Packed 4:2:2 (UYVY or, with `leading_lum`, YUY2) into UYVX, with the
/// linesizes of a 1080p frame.
///
/// The C converts `min(in_linesize, out_linesize) / 2` words of each row,
/// which for the linesizes of a frame in bytes is twice the row: each row
/// reads `4 * width_d2` bytes and writes `8 * width_d2` (the parity test
/// sizes its buffers the same way). The buffers here hold what both sides
/// touch, so the case times that work, not one frame. No caller in OBS uses
/// this conversion (CPU YUV conversion was removed upstream, e5b004fd4).
fn decompress_422(
    criterion: &mut Criterion,
    case: &str,
    leading_lum: bool,
    fns: [Decompress422; 3],
) {
    let (in_linesize, out_linesize) = (WIDTH * 2, WIDTH * 4);
    let width_d2 = in_linesize.min(out_linesize) / 2;
    let input = bytes((HEIGHT - 1) * in_linesize + 4 * width_d2, 5);
    let run = |f: Decompress422, out: &mut Vec<u8>| {
        // SAFETY: `input` and `out` hold every byte the conversion reads
        // and writes for HEIGHT rows (see above).
        unsafe {
            f(
                black_box(input.as_ptr()),
                in_linesize as u32,
                0,
                HEIGHT as u32,
                out.as_mut_ptr(),
                out_linesize as u32,
                leading_lum,
            )
        }
    };
    let [rust, shipped, scalar] = fns;
    let frame = || vec![0u8; (HEIGHT - 1) * out_linesize + 8 * width_d2];
    let (mut r, mut k, mut s) = (frame(), frame(), frame());
    run(rust, &mut r);
    run(shipped, &mut k);
    run(scalar, &mut s);
    assert!(r == k, "benchmark fixture differs: format/{case}");
    assert!(r == s, "benchmark fixture differs: format/{case}_c_scalar");
    super::measure_pair(
        criterion,
        &format!("format/{case}/1080p"),
        || run(rust, &mut r),
        || run(shipped, &mut k),
    );
    super::measure_pair(
        criterion,
        &format!("format/{case}_c_scalar/1080p"),
        || run(rust, &mut r),
        || run(scalar, &mut s),
    );
}

pub fn benchmarks(criterion: &mut Criterion) {
    let (half_w, half_h) = (WIDTH / 2, HEIGHT / 2);
    compress(
        criterion,
        "uyvx_to_i420",
        [(WIDTH, HEIGHT), (half_w, half_h), (half_w, half_h)],
        [WIDTH as u32, half_w as u32],
        [
            rs::compress_uyvx_to_i420,
            c::oracle_compress_uyvx_to_i420,
            c::oracle_scalar_compress_uyvx_to_i420,
        ],
    );
    compress(
        criterion,
        "uyvx_to_nv12",
        [(WIDTH, HEIGHT), (WIDTH, half_h), (0, 0)],
        [WIDTH as u32, WIDTH as u32],
        [
            rs::compress_uyvx_to_nv12,
            c::oracle_compress_uyvx_to_nv12,
            c::oracle_scalar_compress_uyvx_to_nv12,
        ],
    );
    compress(
        criterion,
        "uyvx_to_i444",
        [(WIDTH, HEIGHT), (WIDTH, HEIGHT), (WIDTH, HEIGHT)],
        [WIDTH as u32, WIDTH as u32],
        [
            rs::convert_uyvx_to_i444,
            c::oracle_convert_uyvx_to_i444,
            c::oracle_scalar_convert_uyvx_to_i444,
        ],
    );
    decompress(
        criterion,
        "i420_to_uyvx",
        [(WIDTH, HEIGHT), (half_w, half_h), (half_w, half_h)],
        [
            rs::decompress_420,
            c::oracle_decompress_420,
            c::oracle_scalar_decompress_420,
        ],
    );
    decompress(
        criterion,
        "nv12_to_uyvx",
        [(WIDTH, HEIGHT), (WIDTH, half_h), (0, 0)],
        [
            rs::decompress_nv12,
            c::oracle_decompress_nv12,
            c::oracle_scalar_decompress_nv12,
        ],
    );
    decompress_422(
        criterion,
        "uyvy_to_uyvx",
        false,
        [
            rs::decompress_422,
            c::oracle_decompress_422,
            c::oracle_scalar_decompress_422,
        ],
    );
    decompress_422(
        criterion,
        "yuy2_to_uyvx",
        true,
        [
            rs::decompress_422,
            c::oracle_decompress_422,
            c::oracle_scalar_decompress_422,
        ],
    );
}
