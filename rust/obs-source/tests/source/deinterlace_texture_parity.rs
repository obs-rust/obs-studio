//! Original C and the real field adapter; counts above eight invoke C UB and
//! are excluded. No intentional behavior differences for valid sources.
use core::{
    ffi::c_void,
    mem::{align_of, offset_of, size_of},
};
use obs_c_oracle as _;
use obs_source::ffi::source_deinterlace_texture::{
    Create, Fields, Texrender, allocate_with, set_deinterlace_texture_size,
};
use proptest::prelude::*;
unsafe extern "C" {
    fn oracle_deinterlace_texture_run(
        values: *const u32,
        counts: *const i32,
        fail: i32,
        fail_render: bool,
        run: Option<unsafe extern "C" fn(*mut c_void, Create, Texrender)>,
        out: *mut u64,
    );
    fn oracle_deinterlace_texture_layout(out: *mut usize);
}
/// Run the C and the Rust on the same source, with `counts` (the channel
/// count each allocation leaves behind) or a source nothing changes.
fn run(
    values: [u32; 30],
    counts: Option<&[i32; 9]>,
    fail: i32,
    fail_render: bool,
) -> [[u64; 79]; 2] {
    let mut c = [0; 79];
    let mut rust = [0; 79];
    let counts = counts.map_or(core::ptr::null(), |counts| counts.as_ptr());
    // SAFETY: bounded arrays stay live, and valid channel counts fit the real C arrays.
    unsafe {
        oracle_deinterlace_texture_run(
            values.as_ptr(),
            counts,
            fail,
            fail_render,
            None,
            c.as_mut_ptr(),
        );
        oracle_deinterlace_texture_run(
            values.as_ptr(),
            counts,
            fail,
            fail_render,
            Some(allocate_with),
            rust.as_mut_ptr(),
        );
    }
    [c, rust]
}
fn check(values: [u32; 30], fail: i32, fail_render: bool) {
    let [c, rust] = run(values, None, fail, fail_render);
    assert_eq!(c, rust, "input={values:?}, failure={fail}/{fail_render}");
    assert_eq!(
        rust[78], 1,
        "unrelated real source bytes must stay unchanged"
    );
    for event in rust[4..68]
        .as_chunks::<8>()
        .0
        .iter()
        .take(usize::try_from(rust[68]).unwrap())
    {
        assert_eq!(event[7], 1);
        assert_eq!(event[4], 0);
    }
}
#[test]
fn real_header_layout_and_null_before_adapter() {
    let mut c = [0; 16];
    // SAFETY: layout buffer has the exact output length; NULL is checked before adapter access.
    unsafe {
        oracle_deinterlace_texture_layout(c.as_mut_ptr());
        set_deinterlace_texture_size(core::ptr::null_mut());
    }
    assert_eq!(
        c,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, format),
            offset_of!(Fields, trc),
            offset_of!(Fields, gpu),
            offset_of!(Fields, channels),
            offset_of!(Fields, width),
            offset_of!(Fields, height),
            offset_of!(Fields, widths),
            offset_of!(Fields, heights),
            offset_of!(Fields, formats),
            offset_of!(Fields, textures),
            offset_of!(Fields, render),
            8,
            2,
            0
        ]
    );
}
#[test]
fn characterization_matrix_against_original_c() {
    for format in 0..=25 {
        for trc in [0, 1, 2, 3, 255] {
            for gpu in [0, 1] {
                for channels in 0..=8 {
                    for fail in -1..8 {
                        for fail_render in [false, true] {
                            let mut values = [0; 30];
                            values[..6].copy_from_slice(&[format, trc, gpu, channels, 1920, 1080]);
                            for c in 0..8 {
                                let plane = u32::try_from(c).unwrap();
                                values[6 + c] = 641u32.wrapping_sub(plane * 101);
                                values[14 + c] = 479u32.wrapping_sub(plane * 71);
                                values[22 + c] = if c % 2 == 0 { 18 } else { 2 };
                            }
                            check(values, fail, fail_render);
                        }
                    }
                }
            }
        }
    }
    let mut negative = [0; 30];
    negative[..6].copy_from_slice(&[6, 0, 1, u32::MAX, 0, 0]);
    check(negative, -1, false);
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]
    #[test]
    fn random_real_source_allocation(values in prop::array::uniform30(any::<u32>()), channels in 0i32..=8, raw_channels in any::<i32>(), fail in -1i32..4, fail_render in any::<bool>(), gpu in any::<bool>()) {
        let mut values=values;
        let count = if gpu && raw_channels > 8 {channels} else {raw_channels};
        values[3]=u32::from_ne_bytes(count.to_ne_bytes());
        values[2]=u32::from(gpu);
        // C enum fields accept all 32-bit representations, trc narrows to u8.
        check(values,fail,fail_render);
    }
}

// Each read is made where the C makes it (#2153): the channel count before
// every plane, and each plane's size and format just before its texture,
// after the allocations before it. A source the allocations change shows a
// read made early.
#[test]
fn reads_after_each_allocation_match_c() {
    let mut values = [0; 30];
    values[..6].copy_from_slice(&[0, 0, 1, 3, 1920, 1080]);
    for c in 0..8 {
        let plane = u32::try_from(c).unwrap();
        values[6 + c] = 641 - plane * 61;
        values[14 + c] = 479 - plane * 41;
        values[22 + c] = 2;
    }
    for counts in [
        [3; 9],
        [5; 9],
        [1; 9],
        [0; 9],
        [8; 9],
        [3, 4, 2, 6, 6, 1, 8, 8, 8],
        [8, 7, 6, 5, 4, 3, 2, 1, 0],
    ] {
        for channels in [0, 1, 3, 8] {
            let mut values = values;
            values[3] = channels;
            let [c, rust] = run(values, Some(&counts), -1, false);
            assert_eq!(c, rust, "channels={channels}, counts={counts:?}");
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]
    #[test]
    fn random_reads_after_each_allocation(values in prop::array::uniform30(any::<u32>()), channels in 0u32..=8, counts in prop::array::uniform9(0i32..=8), fail in -1i32..8, fail_render in any::<bool>()) {
        let mut values = values;
        values[2] = 1;
        values[3] = channels;
        let [c, rust] = run(values, Some(&counts), fail, fail_render);
        prop_assert_eq!(c, rust);
    }
}
