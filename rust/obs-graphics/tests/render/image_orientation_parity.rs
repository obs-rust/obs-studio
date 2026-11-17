//! Exact selector parity. The EXIF converter is a deterministic double here:
//! these tests cover both selection branches, not the FFmpeg 8.1 conversion API.
use core::ffi::{CStr, c_char, c_int, c_void};
use obs_graphics::ffi::image_orientation::{gs_ffmpeg_image_get_orientation, select_orientation};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_image_orientation_through_ffi(
        key: *const c_char,
        value: *const c_char,
        data: *const u8,
        size: usize,
        present: c_int,
        select: unsafe extern "C" fn(*const c_void) -> c_int,
    ) -> c_int;
    fn oracle_image_orientation_legacy(
        key: *const c_char,
        value: *const c_char,
        data: *const u8,
        size: usize,
        present: c_int,
    ) -> c_int;
    fn oracle_image_orientation_exif(
        key: *const c_char,
        value: *const c_char,
        data: *const u8,
        size: usize,
        present: c_int,
    ) -> c_int;
}

struct Frame<'a> {
    key: Option<&'a CStr>,
    value: Option<&'a CStr>,
    data: &'a [u8],
    present: bool,
}

unsafe extern "C" fn metadata(frame: *const c_void) -> *const c_char {
    // SAFETY: check() passes a live Frame throughout the synchronous call.
    let frame = unsafe { &*frame.cast::<Frame<'_>>() };
    if frame.key == Some(c"Orientation") {
        frame.value.map_or(core::ptr::null(), CStr::as_ptr)
    } else {
        core::ptr::null()
    }
}

unsafe extern "C" fn matrix(frame: *const c_void, size: *mut usize) -> *const u8 {
    // SAFETY: check() passes a live Frame; the shim provides writable size.
    let frame = unsafe { &*frame.cast::<Frame<'_>>() };
    // SAFETY: size is the shim's local output parameter.
    unsafe { *size = frame.data.len() };
    if frame.present {
        frame.data.as_ptr()
    } else {
        core::ptr::null()
    }
}

unsafe extern "C" fn legacy_convert(matrix: *const i32) -> c_int {
    // SAFETY: shim passes its aligned local nine-element matrix.
    let m = unsafe { &*matrix.cast::<[i32; 9]>() };
    obs_graphics::image_orient::matrix_orientation(m[0], m[1], m[3], m[4])
}

unsafe extern "C" fn exif_convert(matrix: *const i32) -> c_int {
    // SAFETY: shim passes its aligned local nine-element matrix.
    let m = unsafe { &*matrix.cast::<[i32; 9]>() };
    (m.iter()
        .fold(0_u32, |a, &v| a.wrapping_mul(31).wrapping_add(v as u32))
        % 9) as i32
}

fn check(
    key: Option<&CStr>,
    value: Option<&CStr>,
    data: &[u8],
    present: bool,
    modern: bool,
) -> i32 {
    let frame = Frame {
        key,
        value,
        data,
        present,
    };
    let key = key.map_or(core::ptr::null(), CStr::as_ptr);
    let value = value.map_or(core::ptr::null(), CStr::as_ptr);
    // SAFETY: all strings and bytes live through these calls. C only reads
    // 36 bytes when present and length >=36. Both converters read nine i32s.
    let (expected, actual) = unsafe {
        let oracle = if modern {
            oracle_image_orientation_exif
        } else {
            oracle_image_orientation_legacy
        };
        (
            oracle(key, value, data.as_ptr(), data.len(), i32::from(present)),
            select_orientation(
                (&raw const frame).cast(),
                if modern { None } else { Some(metadata) },
                matrix,
                if modern { exif_convert } else { legacy_convert },
            ),
        )
    };
    assert_eq!(actual, expected);
    if !modern {
        // SAFETY: the oracle constructs a live test AVFrame; the real adapter
        // is compiled over the same FFmpeg access doubles in this test build.
        let direct = unsafe {
            oracle_image_orientation_through_ffi(
                key,
                value,
                data.as_ptr(),
                data.len(),
                i32::from(present),
                gs_ffmpeg_image_get_orientation,
            )
        };
        assert_eq!(direct, expected);
    }
    actual
}

#[test]
fn precedence_case_and_atoi_edges() {
    let data = [0_u8; 44];
    for modern in [false, true] {
        for key in [
            None,
            Some(c"orientation"),
            Some(c"ORIENTATION"),
            Some(c"Orientation"),
        ] {
            for value in [
                None,
                Some(c""),
                Some(c"bad"),
                Some(c"  +6tail"),
                Some(c"-7"),
                Some(c"0"),
                Some(c"99"),
            ] {
                for len in 0..=44 {
                    for present in [false, true] {
                        check(key, value, &data[..len], present, modern);
                    }
                }
            }
        }
    }
}

#[test]
fn all_jpeg_orientations_and_missing_metadata() {
    let signs = [
        [1, 0, 0, 1],
        [-1, 0, 0, 1],
        [-1, 0, 0, -1],
        [1, 0, 0, -1],
        [0, 1, 1, 0],
        [0, 1, -1, 0],
        [0, -1, -1, 0],
        [0, -1, 1, 0],
    ];
    for (index, [a, b, c, d]) in signs.into_iter().enumerate() {
        let m: [i32; 9] = [
            a * 65536,
            b * 65536,
            0,
            c * 65536,
            d * 65536,
            0,
            0,
            0,
            1 << 30,
        ];
        let bytes: Vec<_> = [99]
            .into_iter()
            .chain(m.into_iter().flat_map(i32::to_ne_bytes))
            .collect();
        // The offset intentionally tests unaligned FFmpeg side data.
        assert_eq!(
            check(None, None, &bytes[1..], true, false),
            index as i32 + 1
        );
        let value = std::ffi::CString::new((index + 1).to_string()).unwrap();
        assert_eq!(
            check(Some(c"Orientation"), Some(&value), &[], false, false),
            index as i32 + 1
        );
    }
    assert_eq!(check(None, None, &[], false, false), 0);
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(512), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn differential_random_matrices(
        bytes in proptest::collection::vec(any::<u8>(), 45), len in 0usize..=44,
        present in any::<bool>(), modern in any::<bool>(), value in -10000i32..10000,
        key in 0u8..4,
    ) {
        let value = std::ffi::CString::new(value.to_string()).unwrap();
        let key = match key { 0 => None, 1 => Some(c"orientation"), _ => Some(c"Orientation") };
        check(key, Some(&value), &bytes[1..=len], present, modern);
    }
}
