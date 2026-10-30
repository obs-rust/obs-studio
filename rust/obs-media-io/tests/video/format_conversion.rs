//! Tier 1: safe-API tests mirroring `test/cmocka/test_format_conversion.c`,
//! plus edge cases.
//!
//! As in the C test, every output buffer starts filled with `CANARY` plus
//! `SLACK` extra bytes, and each test compares the whole buffer.

use obs_c_oracle as _;
use obs_media_io::format_conversion::{
    compress_uyvx_to_i420, compress_uyvx_to_nv12, convert_uyvx_to_i444, decompress_420,
    decompress_422, decompress_nv12,
};
use proptest as _;

const CANARY: u8 = 0xA5;
const SLACK: usize = 16;

const W: u32 = 8;
const H: u32 = 4;
const UYVX_LINESIZE: u32 = W * 4;

fn canary_buf(size: u32) -> Vec<u8> {
    vec![CANARY; size as usize + SLACK]
}

fn put32(buf: &mut [u8], offset: u32, value: u32) {
    let offset = offset as usize;
    buf[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
}

fn get32(buf: &[u8], offset: usize) -> u32 {
    u32::from_ne_bytes(buf[offset..offset + 4].try_into().unwrap())
}

// Test image channels, as in the C test (`uint8_t` arithmetic wraps).

fn chan_u(x: u32, y: u32) -> u8 {
    (0x10 + 3 * x + 7 * y) as u8
}

fn chan_y(x: u32, y: u32) -> u8 {
    (0x80 + 5 * x + 11 * y) as u8
}

fn chan_v(x: u32, y: u32) -> u8 {
    (0xC0 + 13 * x + 2 * y) as u8
}

/// The chroma average of the 2x2 block at (x, y), rounded down.
fn avg4(chan: fn(u32, u32) -> u8, x: u32, y: u32) -> u8 {
    let sum = u32::from(chan(x, y))
        + u32::from(chan(x + 1, y))
        + u32::from(chan(x, y + 1))
        + u32::from(chan(x + 1, y + 1));
    (sum >> 2) as u8
}

fn make_uyvx(width: u32, height: u32, linesize: u32) -> Vec<u8> {
    let mut buf = canary_buf(linesize * height);
    for y in 0..height {
        for x in 0..width {
            let px = (y * linesize + x * 4) as usize;
            buf[px..px + 4].copy_from_slice(&[chan_u(x, y), chan_y(x, y), chan_v(x, y), 0xEE]);
        }
    }
    buf
}

/// Luma for rows `start_y..end_y` and columns `0..width`.
fn expect_luma(lum: &mut [u8], linesize: u32, width: u32, start_y: u32, end_y: u32) {
    for y in start_y..end_y {
        for x in 0..width {
            lum[(y * linesize + x) as usize] = chan_y(x, y);
        }
    }
}

#[test]
fn test_i420() {
    let input = make_uyvx(W, H, UYVX_LINESIZE);
    let mut output = [
        canary_buf(W * H),
        canary_buf(W * H / 4),
        canary_buf(W * H / 4),
    ];
    let mut expected = output.clone();

    expect_luma(&mut expected[0], W, W, 0, H);
    for cy in 0..H / 2 {
        for cx in 0..W / 2 {
            expected[1][(cy * (W / 2) + cx) as usize] = avg4(chan_u, cx * 2, cy * 2);
            expected[2][(cy * (W / 2) + cx) as usize] = avg4(chan_v, cx * 2, cy * 2);
        }
    }

    let [lum, u, v] = &mut output;
    compress_uyvx_to_i420(
        &input[..],
        UYVX_LINESIZE,
        0,
        H,
        [&mut lum[..], &mut u[..], &mut v[..]],
        &[W, W / 2, W / 2],
    );

    assert_eq!(output, expected);
}

#[test]
fn test_i420_chroma_rounds_down() {
    let mut input = canary_buf(32);
    #[rustfmt::skip]
    let pixels: [u8; 32] = [
        1, 0x10, 10, 0, 2, 0x11, 20, 0, 3, 0x12, 30, 0, 4, 0x13, 40, 0, // row 0
        5, 0x14, 50, 0, 6, 0x15, 61, 0, 7, 0x16, 70, 0, 8, 0x17, 80, 0, // row 1
    ];
    input[..32].copy_from_slice(&pixels);

    // The width is min(in_linesize, out_linesize[0]) = 4 pixels.
    let mut output = [canary_buf(8), canary_buf(4), canary_buf(4)];
    let [lum, u, v] = &mut output;
    compress_uyvx_to_i420(
        &input[..],
        16,
        0,
        2,
        [&mut lum[..], &mut u[..], &mut v[..]],
        &[4, 4, 4],
    );

    assert_eq!(output[0][..4], [0x10, 0x11, 0x12, 0x13]);
    assert_eq!(output[0][4..8], [0x14, 0x15, 0x16, 0x17]);
    assert_eq!(output[0][8], CANARY);
    // U: (1 + 2 + 5 + 6) >> 2 = 3, (3 + 4 + 7 + 8) >> 2 = 5.
    // V: (10 + 20 + 50 + 61) >> 2 = 35, (30 + 40 + 70 + 80) >> 2 = 55.
    assert_eq!(output[1][..3], [3, 5, CANARY]);
    assert_eq!(output[2][..3], [35, 55, CANARY]);
}

#[test]
fn test_i420_rows() {
    let input = make_uyvx(W, H, UYVX_LINESIZE);
    let mut output = [
        canary_buf(W * H),
        canary_buf(W * H / 4),
        canary_buf(W * H / 4),
    ];
    let mut expected = output.clone();

    // Rows 2 and 3 only: chroma row 1.
    expect_luma(&mut expected[0], W, W, 2, 4);
    for cx in 0..W / 2 {
        expected[1][(W / 2 + cx) as usize] = avg4(chan_u, cx * 2, 2);
        expected[2][(W / 2 + cx) as usize] = avg4(chan_v, cx * 2, 2);
    }

    let [lum, u, v] = &mut output;
    compress_uyvx_to_i420(
        &input[..],
        UYVX_LINESIZE,
        2,
        4,
        [&mut lum[..], &mut u[..], &mut v[..]],
        &[W, W / 2, W / 2],
    );

    assert_eq!(output, expected);
}

#[test]
fn test_i420_out_linesize_limits_width() {
    let input = make_uyvx(W, H, UYVX_LINESIZE);
    let mut output = [canary_buf(4 * H), canary_buf(H), canary_buf(H)];
    let mut expected = output.clone();

    expect_luma(&mut expected[0], 4, 4, 0, H);
    for cy in 0..H / 2 {
        for cx in 0..2 {
            expected[1][(cy * 2 + cx) as usize] = avg4(chan_u, cx * 2, cy * 2);
            expected[2][(cy * 2 + cx) as usize] = avg4(chan_v, cx * 2, cy * 2);
        }
    }

    let [lum, u, v] = &mut output;
    compress_uyvx_to_i420(
        &input[..],
        UYVX_LINESIZE,
        0,
        H,
        [&mut lum[..], &mut u[..], &mut v[..]],
        &[4, 2, 2],
    );

    assert_eq!(output, expected);
}

#[test]
fn test_i420_v_plane_uses_u_linesize() {
    let input = make_uyvx(W, H, UYVX_LINESIZE);
    let mut output = [
        canary_buf(W * H),
        canary_buf(W * H / 4),
        canary_buf(W * H / 2),
    ];
    let mut expected = canary_buf(W * H / 2);

    for cy in 0..H / 2 {
        for cx in 0..W / 2 {
            expected[(cy * (W / 2) + cx) as usize] = avg4(chan_v, cx * 2, cy * 2);
        }
    }

    let [lum, u, v] = &mut output;
    compress_uyvx_to_i420(
        &input[..],
        UYVX_LINESIZE,
        0,
        H,
        [&mut lum[..], &mut u[..], &mut v[..]],
        &[W, W / 2, W],
    );

    assert_eq!(output[2], expected);
}

#[test]
fn test_nv12() {
    let input = make_uyvx(W, H, UYVX_LINESIZE);
    let mut output = [canary_buf(W * H), canary_buf(W * H / 2)];
    let mut expected = output.clone();

    expect_luma(&mut expected[0], W, W, 0, H);
    for cy in 0..H / 2 {
        for cx in 0..W / 2 {
            expected[1][(cy * W + cx * 2) as usize] = avg4(chan_u, cx * 2, cy * 2);
            expected[1][(cy * W + cx * 2 + 1) as usize] = avg4(chan_v, cx * 2, cy * 2);
        }
    }

    let [lum, chroma] = &mut output;
    compress_uyvx_to_nv12(
        &input[..],
        UYVX_LINESIZE,
        0,
        H,
        [&mut lum[..], &mut chroma[..]],
        &[W, W],
    );

    assert_eq!(output, expected);
}

#[test]
fn test_nv12_rows_and_linesize() {
    let input = make_uyvx(W, H, UYVX_LINESIZE);
    let mut output = [canary_buf(W * H), canary_buf((W + 8) * H / 2)];
    let mut expected = output.clone();

    expect_luma(&mut expected[0], W, W, 2, 4);
    for cx in 0..W / 2 {
        expected[1][(W + 8 + cx * 2) as usize] = avg4(chan_u, cx * 2, 2);
        expected[1][(W + 8 + cx * 2 + 1) as usize] = avg4(chan_v, cx * 2, 2);
    }

    let [lum, chroma] = &mut output;
    compress_uyvx_to_nv12(
        &input[..],
        UYVX_LINESIZE,
        2,
        4,
        [&mut lum[..], &mut chroma[..]],
        &[W, W + 8],
    );

    assert_eq!(output, expected);
}

#[test]
fn test_i444() {
    let input = make_uyvx(W, H, UYVX_LINESIZE);
    let mut output = [canary_buf(W * H), canary_buf(W * H), canary_buf(W * H)];
    let mut expected = output.clone();

    expect_luma(&mut expected[0], W, W, 0, H);
    for y in 0..H {
        for x in 0..W {
            expected[1][(y * W + x) as usize] = chan_u(x, y);
            expected[2][(y * W + x) as usize] = chan_v(x, y);
        }
    }

    let [lum, u, v] = &mut output;
    convert_uyvx_to_i444(
        &input[..],
        UYVX_LINESIZE,
        0,
        H,
        [&mut lum[..], &mut u[..], &mut v[..]],
        W,
    );

    assert_eq!(output, expected);
}

/// The C test passes out_linesize {W, 1, 2}; only the first is read.
#[test]
fn test_i444_planes_use_luma_linesize() {
    let input = make_uyvx(W, H, UYVX_LINESIZE);
    let mut output = [canary_buf(W * H), canary_buf(W * H), canary_buf(W * H)];
    let mut expected = output.clone();

    expect_luma(&mut expected[0], W, W, 2, 4);
    for y in 2..4 {
        for x in 0..W {
            expected[1][(y * W + x) as usize] = chan_u(x, y);
            expected[2][(y * W + x) as usize] = chan_v(x, y);
        }
    }

    let [lum, u, v] = &mut output;
    convert_uyvx_to_i444(
        &input[..],
        UYVX_LINESIZE,
        2,
        4,
        [&mut lum[..], &mut u[..], &mut v[..]],
        W,
    );

    assert_eq!(output, expected);
}

fn make_plane(chan: fn(u32, u32) -> u8, width: u32, height: u32, linesize: u32) -> Vec<u8> {
    let mut buf = canary_buf(linesize * height);
    for y in 0..height {
        for x in 0..width {
            buf[(y * linesize + x) as usize] = chan(x, y);
        }
    }
    buf
}

/// `Y << 16 | U << 8 | V`, with U and V shared by each 2x2 block.
fn i420_pixel(x: u32, y: u32) -> u32 {
    u32::from(chan_y(x, y)) << 16
        | u32::from(chan_u(x / 2, y / 2)) << 8
        | u32::from(chan_v(x / 2, y / 2))
}

#[test]
fn test_decompress_420() {
    let out_linesize = W * 4 + 8;
    let in_linesize = [W, W / 2 + 2, W / 2 + 4];
    let planes = [
        make_plane(chan_y, W, H, in_linesize[0]),
        make_plane(chan_u, W / 2, H / 2, in_linesize[1]),
        make_plane(chan_v, W / 2, H / 2, in_linesize[2]),
    ];
    let mut output = canary_buf(out_linesize * H);
    let mut expected = output.clone();

    for y in 0..H {
        for x in 0..W {
            put32(&mut expected, y * out_linesize + x * 4, i420_pixel(x, y));
        }
    }

    decompress_420(
        [&planes[0][..], &planes[1][..], &planes[2][..]],
        &in_linesize,
        0,
        H,
        &mut output[..],
        out_linesize,
    );

    assert_eq!(output, expected);
}

#[test]
fn test_decompress_420_odd_rows_round_down() {
    let out_linesize = W * 4;
    let in_linesize = [W, W / 2, W / 2];
    let planes = [
        make_plane(chan_y, W, H, in_linesize[0]),
        make_plane(chan_u, W / 2, H / 2, in_linesize[1]),
        make_plane(chan_v, W / 2, H / 2, in_linesize[2]),
    ];
    let input = [&planes[0][..], &planes[1][..], &planes[2][..]];
    let mut output = canary_buf(out_linesize * H);
    let mut expected = output.clone();

    for y in 0..2 {
        for x in 0..W {
            put32(&mut expected, y * out_linesize + x * 4, i420_pixel(x, y));
        }
    }

    decompress_420(input, &in_linesize, 1, 3, &mut output[..], out_linesize);
    assert_eq!(output, expected);

    // start_y 2, end_y 3: 1 / 1, nothing.
    decompress_420(input, &in_linesize, 2, 3, &mut output[..], out_linesize);
    assert_eq!(output, expected);
}

#[test]
fn test_decompress_420_width_from_luma_linesize() {
    let out_linesize = W * 4;
    let in_linesize = [7, 4, 4];
    let planes = [
        make_plane(chan_y, 7, 2, in_linesize[0]),
        make_plane(chan_u, 4, 1, in_linesize[1]),
        make_plane(chan_v, 4, 1, in_linesize[2]),
    ];
    let mut output = canary_buf(out_linesize * 2);
    let mut expected = output.clone();

    for y in 0..2 {
        for x in 0..6 {
            put32(&mut expected, y * out_linesize + x * 4, i420_pixel(x, y));
        }
    }

    decompress_420(
        [&planes[0][..], &planes[1][..], &planes[2][..]],
        &in_linesize,
        0,
        2,
        &mut output[..],
        out_linesize,
    );

    assert_eq!(output, expected);
}

fn make_uv_plane(width: u32, height: u32, linesize: u32) -> Vec<u8> {
    let mut buf = canary_buf(linesize * height);
    for y in 0..height {
        for x in 0..width {
            let i = (y * linesize + x * 2) as usize;
            buf[i] = chan_u(x, y);
            buf[i + 1] = chan_v(x, y);
        }
    }
    buf
}

/// `Y | uv << 8`, where `uv` is the U V byte pair read as a native `u16`.
fn nv12_pixel(x: u32, y: u32) -> u32 {
    let uv = u16::from_ne_bytes([chan_u(x / 2, y / 2), chan_v(x / 2, y / 2)]);
    u32::from(chan_y(x, y)) | u32::from(uv) << 8
}

#[test]
fn test_decompress_nv12() {
    let out_linesize = W * 4 + 8;
    let in_linesize = [W, W + 4];
    let planes = [
        make_plane(chan_y, W, H, in_linesize[0]),
        make_uv_plane(W / 2, H / 2, in_linesize[1]),
    ];
    let mut output = canary_buf(out_linesize * H);
    let mut expected = output.clone();

    for y in 0..H {
        for x in 0..W {
            put32(&mut expected, y * out_linesize + x * 4, nv12_pixel(x, y));
        }
    }

    decompress_nv12(
        [&planes[0][..], &planes[1][..]],
        &in_linesize,
        0,
        H,
        &mut output[..],
        out_linesize,
    );

    assert_eq!(output, expected);
}

#[test]
fn test_decompress_nv12_rows() {
    let out_linesize = W * 4;
    let in_linesize = [W, W];
    let planes = [
        make_plane(chan_y, W, H, in_linesize[0]),
        make_uv_plane(W / 2, H / 2, in_linesize[1]),
    ];
    let mut output = canary_buf(out_linesize * H);
    let mut expected = output.clone();

    // start_y 3, end_y 4: 1 / 2, so rows 2 and 3.
    for y in 2..4 {
        for x in 0..W {
            put32(&mut expected, y * out_linesize + x * 4, nv12_pixel(x, y));
        }
    }

    decompress_nv12(
        [&planes[0][..], &planes[1][..]],
        &in_linesize,
        3,
        4,
        &mut output[..],
        out_linesize,
    );

    assert_eq!(output, expected);
}

/// As the C test's `dup_luma`.
fn dup_luma(dw: u32, leading_lum: bool) -> u32 {
    if leading_lum {
        (dw & 0xFFFF_FF00) | ((dw >> 16) & 0xFF)
    } else {
        (dw & 0xFFFF_00FF) | ((dw >> 16) & 0xFF00)
    }
}

#[test]
fn test_decompress_422_literals() {
    assert_eq!(dup_luma(0x4433_2211, true), 0x4433_2233);
    assert_eq!(dup_luma(0x4433_2211, false), 0x4433_4411);

    for leading in [false, true] {
        let mut input = canary_buf(8);
        let mut output = canary_buf(16);
        put32(&mut input, 0, 0x4433_2211);
        put32(&mut input, 4, 0x8877_6655);

        // in_linesize 4, out_linesize 8: min(4, 8) / 4 = 1 dword in, 2 out;
        // the second input dword is not read.
        decompress_422(&input[..], 4, 0, 1, &mut output[..], 8, leading);

        assert_eq!(get32(&output, 0), 0x4433_2211);
        let second = if leading { 0x4433_2233 } else { 0x4433_4411 };
        assert_eq!(get32(&output, 4), second);
        assert_eq!(get32(&output, 8), u32::from_ne_bytes([CANARY; 4]));
        assert_eq!(get32(&output, 12), u32::from_ne_bytes([CANARY; 4]));
        assert_eq!(output[16], CANARY);
    }
}

#[test]
fn test_decompress_422_rows_in_bounds() {
    let dwords: [u32; 6] = [
        0x4433_2211,
        0x8877_6655,
        0xCCBB_AA99,
        0x00FF_EEDD,
        0x1357_9BDF,
        0x2468_ACE0,
    ];

    for leading in [false, true] {
        let input: Vec<u8> = dwords.iter().flat_map(|d| d.to_ne_bytes()).collect();
        let mut output = canary_buf(48);
        let mut expected = output.clone();

        for (i, &dw) in (0u32..).zip(&dwords[..4]) {
            put32(&mut expected, i * 8, dw);
            put32(&mut expected, i * 8 + 4, dup_luma(dw, leading));
        }

        decompress_422(&input[..], 8, 0, 2, &mut output[..], 16, leading);

        assert_eq!(output, expected);
    }
}

#[test]
fn test_decompress_422_one_row() {
    for leading in [false, true] {
        let mut input = canary_buf(16);
        let mut output = canary_buf(16);
        let mut expected = canary_buf(16);
        put32(&mut input, 0, 0x4433_2211);
        put32(&mut input, 4, 0x8877_6655);
        put32(&mut input, 8, 0xCCBB_AA99);
        put32(&mut input, 12, 0x00FF_EEDD);
        put32(&mut expected, 0, 0x4433_2211);
        put32(&mut expected, 4, dup_luma(0x4433_2211, leading));
        put32(&mut expected, 8, 0x8877_6655);
        put32(&mut expected, 12, dup_luma(0x8877_6655, leading));

        decompress_422(&input[..], 8, 0, 1, &mut output[..], 16, leading);

        assert_eq!(output, expected);
    }
}

#[test]
fn test_empty_row_range() {
    let uyvx = make_uyvx(W, H, UYVX_LINESIZE);
    let planes = [
        make_plane(chan_y, W, H, W),
        make_uv_plane(W / 2, H / 2, W),
        make_plane(chan_v, W, H, W),
    ];
    let mut output = [
        canary_buf(W * H * 4),
        canary_buf(W * H * 4),
        canary_buf(W * H * 4),
    ];
    let expected = canary_buf(W * H * 4);
    let linesizes = [W, W, W];

    let [a, b, c] = &mut output;
    compress_uyvx_to_i420(
        &uyvx[..],
        UYVX_LINESIZE,
        2,
        2,
        [&mut a[..], &mut b[..], &mut c[..]],
        &linesizes,
    );
    compress_uyvx_to_nv12(
        &uyvx[..],
        UYVX_LINESIZE,
        2,
        2,
        [&mut a[..], &mut b[..]],
        &linesizes,
    );
    convert_uyvx_to_i444(
        &uyvx[..],
        UYVX_LINESIZE,
        2,
        2,
        [&mut a[..], &mut b[..], &mut c[..]],
        W,
    );
    let input = [&planes[0][..], &planes[1][..], &planes[2][..]];
    decompress_420(input, &linesizes, 2, 2, &mut a[..], W * 4);
    decompress_nv12([input[0], input[1]], &linesizes, 2, 2, &mut a[..], W * 4);
    decompress_422(input[0], W, 2, 2, &mut a[..], W * 4, true);
    decompress_422(input[0], W, 2, 2, &mut a[..], W * 4, false);

    for buf in &output {
        assert_eq!(*buf, expected);
    }
}

// Edge cases beyond the C test.

/// A zero linesize gives a zero width: rows are visited, nothing is read or
/// written.
#[test]
fn zero_width_writes_nothing() {
    let empty: [u8; 0] = [];
    let mut output = [canary_buf(0), canary_buf(0), canary_buf(0)];
    let [a, b, c] = &mut output;

    compress_uyvx_to_i420(
        &empty[..],
        0,
        0,
        H,
        [&mut a[..], &mut b[..], &mut c[..]],
        &[W, W, W],
    );
    compress_uyvx_to_nv12(
        &empty[..],
        UYVX_LINESIZE,
        0,
        H,
        [&mut a[..], &mut b[..]],
        &[0, W],
    );
    convert_uyvx_to_i444(&empty[..], 0, 0, H, [&mut a[..], &mut b[..], &mut c[..]], W);
    decompress_420([&empty[..]; 3], &[0, W, W], 0, H, &mut a[..], W * 4);
    decompress_nv12([&empty[..]; 2], &[W, W], 0, H, &mut a[..], 0);
    decompress_422(&empty[..], W, 0, H, &mut a[..], 0, true);

    assert!(output.iter().all(|buf| buf.iter().all(|&b| b == CANARY)));
}

/// Chroma sums stay in range: four 255s average to 255, four 0s to 0.
#[test]
fn chroma_average_extremes() {
    // Pixels 0 and 1 of both rows are U = V = 255, pixels 2 and 3 zero.
    let mut input = [0u8; 32];
    for px in [0, 4, 16, 20] {
        input[px..px + 4].copy_from_slice(&[0xFF, 0, 0xFF, 0]);
    }
    let mut output = [canary_buf(8), canary_buf(4), canary_buf(4)];
    let [lum, u, v] = &mut output;

    compress_uyvx_to_i420(
        &input[..],
        16,
        0,
        2,
        [&mut lum[..], &mut u[..], &mut v[..]],
        &[4, 4, 4],
    );

    assert_eq!(output[1][..2], [0xFF, 0]);
    assert_eq!(output[2][..2], [0xFF, 0]);
}

/// With overlapping output rows the C's write order decides: in
/// `decompress_420` each step writes two pixels of the first row, then two
/// of the second, so with `out_linesize` 0 the second row wins.
#[test]
fn decompress_420_overlapping_rows_keep_c_order() {
    let in_linesize = [W, W / 2, W / 2];
    let planes = [
        make_plane(chan_y, W, 2, in_linesize[0]),
        make_plane(chan_u, W / 2, 1, in_linesize[1]),
        make_plane(chan_v, W / 2, 1, in_linesize[2]),
    ];
    let mut output = canary_buf(W * 4);
    let mut expected = output.clone();
    for x in 0..W {
        put32(&mut expected, x * 4, i420_pixel(x, 1));
    }

    decompress_420(
        [&planes[0][..], &planes[1][..], &planes[2][..]],
        &in_linesize,
        0,
        2,
        &mut output[..],
        0,
    );

    assert_eq!(output, expected);
}

/// Row offsets are `u32` products, wrapping as in the C: row 2 of a
/// 2^31-byte linesize starts at offset 0.
#[test]
fn decompress_422_row_offset_wraps_like_c() {
    let input: Vec<u8> = (0..16).collect();
    let mut output = canary_buf(48);

    decompress_422(&input[..], 1 << 31, 2, 3, &mut output[..], 8, true);

    // out_linesize 8: row 2 writes at 16, min(2^31, 8) / 4 = 2 dwords in.
    assert!(output[..16].iter().all(|&b| b == CANARY));
    assert!(output[32..].iter().all(|&b| b == CANARY));
    for i in 0..2 {
        let dw = get32(&input, i * 4);
        assert_eq!(get32(&output, 16 + i * 8), dw);
        assert_eq!(get32(&output, 16 + i * 8 + 4), dup_luma(dw, true));
    }
}

/// Byte slices panic where the C would write out of bounds.
#[test]
#[should_panic(expected = "out of range")]
fn short_output_panics() {
    let input = make_uyvx(W, H, UYVX_LINESIZE);
    let mut lum = vec![0u8; (W * H - 1) as usize];
    let mut u = [0u8; 64];
    let mut v = [0u8; 64];

    compress_uyvx_to_i420(
        &input[..],
        UYVX_LINESIZE,
        0,
        H,
        [&mut lum[..], &mut u[..], &mut v[..]],
        &[W, W / 2, W / 2],
    );
}
