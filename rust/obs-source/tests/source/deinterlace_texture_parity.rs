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
        fail: i32,
        fail_render: bool,
        run: Option<unsafe extern "C" fn(*mut c_void, Create, Texrender)>,
        out: *mut u64,
    );
    fn oracle_deinterlace_texture_layout(out: *mut usize);
}
fn check(values: [u32; 30], fail: i32, fail_render: bool) {
    let mut c = [0; 79];
    let mut rust = [0; 79];
    // SAFETY: bounded arrays stay live, and valid channel counts fit the real C arrays.
    unsafe {
        oracle_deinterlace_texture_run(values.as_ptr(), fail, fail_render, None, c.as_mut_ptr());
        oracle_deinterlace_texture_run(
            values.as_ptr(),
            fail,
            fail_render,
            Some(allocate_with),
            rust.as_mut_ptr(),
        );
    }
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
