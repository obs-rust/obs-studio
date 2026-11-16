//! Tier 3: `gs_create_texture_file_data2` against the original
//! `texture-file-data2.c.h`, compiled unchanged as
//! `oracle_gs_create_texture_file_data2` (oracle/texture_file_data2.c).
//!
//! Both sides call the same thread-local recording data3
//! (`oracle_tfd2_data3`): the original through a rename of its one call,
//! the Rust through [`create_texture_file_data2_with`], the function the
//! export wraps. A case compares the result, how often data3 ran, the
//! file, alpha mode and output pointers it got, and the caller's outputs
//! afterwards. Data3's colour-space pointer is each side's own local, so
//! it is compared by kind: non-null, writable, and none of the caller's.
//! Full-width alpha modes, formats, sizes and colour spaces, null and
//! arbitrary file and result pointers, with data3 writing or not.
//!
//! `mutants_are_caught` runs the same comparison over broken shims and
//! requires each to differ from the C. `export_over_ffmpeg_stand_in` runs
//! the real export over the crate's data3 stand-in in a child process,
//! with real `bmalloc` blocks, against data3 itself.
//!
//! No intentional differences. No owned threads; the child process is the
//! only one started.

use core::ffi::{CStr, c_char};
use core::ptr;
use std::ffi::CString;

use obs_c_oracle as _;
use obs_graphics::ffi::texture_file_data2::{
    CreateTextureFileData3, create_texture_file_data2_with, gs_create_texture_file_data2,
};
use obs_graphics::image_file::ImageAlphaMode;
use obs_graphics::texture_render::{ColorFormat, ColorSpace};
use obs_util::ffi::bmem::{bfree, bnum_allocs};
use proptest::prelude::*;

/// `struct oracle_tfd2_call`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Call {
    file: usize,
    alpha_mode: u32,
    format: usize,
    cx_out: usize,
    cy_out: usize,
    space: usize,
}

unsafe extern "C" {
    fn oracle_gs_create_texture_file_data2(
        file: *const c_char,
        alpha_mode: ImageAlphaMode,
        format: *mut ColorFormat,
        cx_out: *mut u32,
        cy_out: *mut u32,
    ) -> *mut u8;
    fn oracle_tfd2_data3(
        file: *const c_char,
        alpha_mode: ImageAlphaMode,
        format: *mut ColorFormat,
        cx_out: *mut u32,
        cy_out: *mut u32,
        space: *mut ColorSpace,
    ) -> *mut u8;
    fn oracle_tfd2_configure(
        result: *mut u8,
        write: bool,
        format: u32,
        cx: u32,
        cy: u32,
        space: u32,
    );
    fn oracle_tfd2_calls(last: *mut Call) -> u32;
    fn oracle_tfd2_layout(out: *mut usize, cap: usize) -> usize;

    /// The crate's FFmpeg stand-in (oracle/texture_render_device.c).
    fn gs_create_texture_file_data3(
        file: *const c_char,
        alpha_mode: ImageAlphaMode,
        format: *mut ColorFormat,
        cx_out: *mut u32,
        cy_out: *mut u32,
        space: *mut ColorSpace,
    ) -> *mut u8;
}

/// A data2 implementation over a given data3.
type Data2 = unsafe fn(
    CreateTextureFileData3,
    *const c_char,
    ImageAlphaMode,
    *mut ColorFormat,
    *mut u32,
    *mut u32,
) -> *mut u8;

/// The original C; its data3 is fixed to the recorder at compile time.
unsafe fn original(
    _data3: CreateTextureFileData3,
    file: *const c_char,
    alpha_mode: ImageAlphaMode,
    format: *mut ColorFormat,
    cx_out: *mut u32,
    cy_out: *mut u32,
) -> *mut u8 {
    // SAFETY: the recorder dereferences only the output pointers.
    unsafe { oracle_gs_create_texture_file_data2(file, alpha_mode, format, cx_out, cy_out) }
}

#[derive(Clone, Copy, Debug)]
struct Case {
    file: usize,
    alpha_mode: u32,
    result: usize,
    write: bool,
    format: u32,
    cx: u32,
    cy: u32,
    space: u32,
    before: (u32, u32, u32),
}

/// Everything a run shows the caller and data3, with data3's colour-space
/// pointer replaced by its kind.
#[derive(Debug, PartialEq, Eq)]
struct Observation {
    result: usize,
    calls: u32,
    file: usize,
    alpha_mode: u32,
    format_is_callers: bool,
    cx_is_callers: bool,
    cy_is_callers: bool,
    space_is_private: bool,
    after: (u32, u32, u32),
    caller_space: u32,
}

fn run(data2: Data2, case: &Case) -> Observation {
    let mut format = ColorFormat(case.before.0);
    let mut cx = case.before.1;
    let mut cy = case.before.2;
    // A colour space the caller owns, to show data2 never writes it.
    let caller_space = ColorSpace(0x00c0_ffee);
    let mut call = Call::default();
    let (format_p, cx_p, cy_p) = (
        ptr::from_mut(&mut format),
        ptr::from_mut(&mut cx),
        ptr::from_mut(&mut cy),
    );
    // SAFETY: the recorder is thread-local; it never dereferences the file
    // or the result, and writes only through the live output pointers.
    let (result, calls) = unsafe {
        oracle_tfd2_configure(
            ptr::without_provenance_mut(case.result),
            case.write,
            case.format,
            case.cx,
            case.cy,
            case.space,
        );
        let result = data2(
            oracle_tfd2_data3,
            ptr::without_provenance(case.file),
            ImageAlphaMode(case.alpha_mode),
            format_p,
            cx_p,
            cy_p,
        );
        (result.addr(), oracle_tfd2_calls(&raw mut call))
    };
    let callers = [
        format_p.addr(),
        cx_p.addr(),
        cy_p.addr(),
        ptr::from_ref(&caller_space).addr(),
    ];
    Observation {
        result,
        calls,
        file: call.file,
        alpha_mode: call.alpha_mode,
        format_is_callers: call.format == format_p.addr(),
        cx_is_callers: call.cx_out == cx_p.addr(),
        cy_is_callers: call.cy_out == cy_p.addr(),
        space_is_private: call.space != 0 && !callers.contains(&call.space),
        after: (format.0, cx, cy),
        caller_space: caller_space.0,
    }
}

unsafe fn rust(
    data3: CreateTextureFileData3,
    file: *const c_char,
    alpha_mode: ImageAlphaMode,
    format: *mut ColorFormat,
    cx_out: *mut u32,
    cy_out: *mut u32,
) -> *mut u8 {
    // SAFETY: as `original`.
    unsafe { create_texture_file_data2_with(data3, file, alpha_mode, format, cx_out, cy_out) }
}

fn compare(case: &Case) {
    let expected = run(original, case);
    let actual = run(rust, case);
    assert_eq!(actual, expected, "{case:?}");
    // What the original shows, so the comparison cannot pass vacuously.
    assert_eq!(expected.calls, 1);
    assert_eq!(expected.result, case.result);
    assert_eq!(expected.file, case.file);
    assert_eq!(expected.alpha_mode, case.alpha_mode);
    assert!(expected.format_is_callers && expected.cx_is_callers && expected.cy_is_callers);
    assert!(expected.space_is_private);
    let after = if case.write {
        (case.format, case.cx, case.cy)
    } else {
        case.before
    };
    assert_eq!(expected.after, after);
    assert_eq!(expected.caller_space, 0x00c0_ffee);
}

/// The characterization's cases: the three alpha modes, success writing
/// RGBA 3x2 sRGB and failure writing nothing, from the C test's sentinels.
fn characterization() -> Vec<Case> {
    let mut cases = Vec::new();
    for alpha_mode in 0..3 {
        for (write, result) in [(true, 0x1000), (false, 0)] {
            cases.push(Case {
                file: 0x2000,
                alpha_mode,
                result,
                write,
                format: 3,
                cx: 3,
                cy: 2,
                space: 0,
                before: (5, 91, 92),
            });
        }
    }
    cases
}

/// Edges: null and extreme pointers, every full-width boundary value, a
/// result with no outputs written and outputs written with no result.
fn edges() -> Vec<Case> {
    let words = [0, 1, 2, 3, 0x7fff_ffff, 0x8000_0000, u32::MAX];
    let mut cases = Vec::new();
    for (i, &w) in words.iter().enumerate() {
        for write in [false, true] {
            for (file, result) in [(0, 0), (usize::MAX, usize::MAX), (0, 1), (1, 0)] {
                cases.push(Case {
                    file,
                    alpha_mode: w,
                    result,
                    write,
                    format: w,
                    cx: words[(i + 1) % words.len()],
                    cy: words[(i + 2) % words.len()],
                    space: words[(i + 3) % words.len()],
                    before: (u32::MAX - w, w, !w),
                });
            }
        }
    }
    cases
}

#[test]
fn layout_matches_c() {
    let mut out = [0usize; 8];
    // SAFETY: writes at most `out.len()` values.
    let n = unsafe { oracle_tfd2_layout(out.as_mut_ptr(), out.len()) };
    assert_eq!(n, 8);
    let rust = [
        size_of::<ImageAlphaMode>(),
        align_of::<ImageAlphaMode>(),
        size_of::<ColorFormat>(),
        align_of::<ColorFormat>(),
        size_of::<ColorSpace>(),
        align_of::<ColorSpace>(),
        size_of::<Call>(),
        align_of::<Call>(),
    ];
    assert_eq!(out, rust);
}

#[test]
fn characterization_matches_c() {
    for case in characterization() {
        compare(&case);
    }
}

#[test]
fn edges_match_c() {
    for case in edges() {
        compare(&case);
    }
}

fn case() -> impl Strategy<Value = Case> {
    let pointer = prop_oneof![Just(0usize), Just(usize::MAX), any::<usize>()];
    (
        (pointer.clone(), any::<u32>(), pointer, any::<bool>()),
        (any::<u32>(), any::<u32>(), any::<u32>(), any::<u32>()),
        (any::<u32>(), any::<u32>(), any::<u32>()),
    )
        .prop_map(
            |((file, alpha_mode, result, write), (format, cx, cy, space), before)| Case {
                file,
                alpha_mode,
                result,
                write,
                format,
                cx,
                cy,
                space,
                before,
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(512),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn random_calls_match_c(case in case()) {
        compare(&case);
    }
}

// Broken shims. Each must differ from the original on some case above.

unsafe fn twice(
    d: CreateTextureFileData3,
    f: *const c_char,
    a: ImageAlphaMode,
    o: *mut ColorFormat,
    x: *mut u32,
    y: *mut u32,
) -> *mut u8 {
    let mut s = ColorSpace::default();
    // SAFETY: the recorder's contract, as the real shim.
    unsafe {
        d(f, a, o, x, y, &raw mut s);
        d(f, a, o, x, y, &raw mut s)
    }
}

unsafe fn drops_result(
    d: CreateTextureFileData3,
    f: *const c_char,
    a: ImageAlphaMode,
    o: *mut ColorFormat,
    x: *mut u32,
    y: *mut u32,
) -> *mut u8 {
    // SAFETY: as the real shim.
    unsafe { create_texture_file_data2_with(d, f, a, o, x, y) };
    ptr::null_mut()
}

unsafe fn null_space(
    d: CreateTextureFileData3,
    f: *const c_char,
    a: ImageAlphaMode,
    o: *mut ColorFormat,
    x: *mut u32,
    y: *mut u32,
) -> *mut u8 {
    // The mutant writes through null only if data3 writes; the cases it
    // runs on here have data3 not writing.
    // SAFETY: see above; the comparison only uses non-writing cases.
    unsafe { d(f, a, o, x, y, ptr::null_mut()) }
}

unsafe fn space_aliases_format(
    d: CreateTextureFileData3,
    f: *const c_char,
    a: ImageAlphaMode,
    o: *mut ColorFormat,
    x: *mut u32,
    y: *mut u32,
) -> *mut u8 {
    // SAFETY: ColorFormat and ColorSpace are both repr(transparent) u32.
    unsafe { d(f, a, o, x, y, o.cast()) }
}

unsafe fn swaps_sizes(
    d: CreateTextureFileData3,
    f: *const c_char,
    a: ImageAlphaMode,
    o: *mut ColorFormat,
    x: *mut u32,
    y: *mut u32,
) -> *mut u8 {
    // SAFETY: as the real shim.
    unsafe { create_texture_file_data2_with(d, f, a, o, y, x) }
}

unsafe fn narrows_alpha(
    d: CreateTextureFileData3,
    f: *const c_char,
    a: ImageAlphaMode,
    o: *mut ColorFormat,
    x: *mut u32,
    y: *mut u32,
) -> *mut u8 {
    // SAFETY: as the real shim.
    unsafe { create_texture_file_data2_with(d, f, ImageAlphaMode(a.0 & 0xff), o, x, y) }
}

unsafe fn null_file(
    d: CreateTextureFileData3,
    _f: *const c_char,
    a: ImageAlphaMode,
    o: *mut ColorFormat,
    x: *mut u32,
    y: *mut u32,
) -> *mut u8 {
    // SAFETY: as the real shim.
    unsafe { create_texture_file_data2_with(d, ptr::null(), a, o, x, y) }
}

#[test]
fn mutants_are_caught() {
    let mutants: [(&str, Data2); 7] = [
        ("twice", twice),
        ("drops_result", drops_result),
        ("null_space", null_space),
        ("space_aliases_format", space_aliases_format),
        ("swaps_sizes", swaps_sizes),
        ("narrows_alpha", narrows_alpha),
        ("null_file", null_file),
    ];
    let cases: Vec<Case> = characterization().into_iter().chain(edges()).collect();
    for (name, mutant) in mutants {
        let caught = cases
            .iter()
            .filter(|case| name != "null_space" || !case.write)
            .any(|case| run(mutant, case) != run(original, case));
        assert!(caught, "mutant {name} survives the differential");
    }
}

fn fixture(name: &str) -> CString {
    CString::new(format!(
        "{}/../../test/cmocka/data/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

/// `bnum_allocs`, whose `long` is 32 bits on Windows.
#[allow(clippy::useless_conversion)]
fn allocs_now() -> i64 {
    // SAFETY: reads the process-wide counter.
    i64::from(unsafe { bnum_allocs() })
}

/// The real export over the stand-in data3, which decodes an existing file
/// to a 3x2 RGBA `bmalloc` block and fails a missing one without writing.
fn export_parity() {
    let files = [
        fixture("image-file.png"),
        fixture("texture-data2-does-not-exist.png"),
        CString::new("").unwrap(),
    ];
    for _ in 0..8 {
        for file in &files {
            for alpha in [0, 1, 2, u32::MAX] {
                let base = allocs_now();
                let (mut format2, mut cx2, mut cy2) = (ColorFormat(5), 91, 92);
                let (mut format3, mut cx3, mut cy3) = (ColorFormat(5), 91, 92);
                let mut space = ColorSpace(2);
                // SAFETY: a NUL-terminated path and live outputs; each
                // non-null result is a bmalloc block freed below.
                unsafe {
                    let data2 = gs_create_texture_file_data2(
                        file.as_ptr(),
                        ImageAlphaMode(alpha),
                        &raw mut format2,
                        &raw mut cx2,
                        &raw mut cy2,
                    );
                    let data3 = gs_create_texture_file_data3(
                        file.as_ptr(),
                        ImageAlphaMode(alpha),
                        &raw mut format3,
                        &raw mut cx3,
                        &raw mut cy3,
                        &raw mut space,
                    );
                    assert_eq!(data2.is_null(), data3.is_null(), "{file:?}");
                    assert_eq!((format2, cx2, cy2), (format3, cx3, cy3));
                    if data2.is_null() {
                        assert_eq!((format2, cx2, cy2), (ColorFormat(5), 91, 92));
                        assert_eq!(space, ColorSpace(2));
                    } else {
                        assert_ne!(data2, data3);
                        assert_eq!((format2, cx2, cy2), (ColorFormat(3), 3, 2));
                        assert_eq!(space, ColorSpace(0));
                        assert_eq!(
                            core::slice::from_raw_parts(data2, 24),
                            core::slice::from_raw_parts(data3, 24)
                        );
                        assert_eq!(allocs_now(), base + 2);
                    }
                    bfree(data2.cast());
                    bfree(data3.cast());
                }
                assert_eq!(allocs_now(), base, "{file:?} alpha {alpha}");
            }
        }
    }
    // The stand-in recorded one data3 per call on each side.
    // SAFETY: the stand-in's log is a NUL-terminated static buffer.
    let log =
        unsafe { CStr::from_ptr(obs_c_oracle::texture_render::oracle_texrender_device_log()) }
            .to_string_lossy()
            .into_owned();
    assert_eq!(log.matches("file_data ").count(), 8 * 3 * 4 * 2, "{log}");
}

#[test]
fn export_over_ffmpeg_stand_in() {
    const CHILD: &str = "OBS_TEXTURE_FILE_DATA2_PARITY_CHILD";
    if std::env::var_os(CHILD).is_some() {
        // SAFETY: this child is the only user of the process-wide stand-in.
        unsafe { obs_c_oracle::texture_render::oracle_texrender_device_reset() };
        export_parity();
        return;
    }
    // The stand-in's log and the allocation counter are process-wide:
    // re-exec this exact case alone, in this same test executable.
    let name = format!("{}::export_over_ffmpeg_stand_in", module_path!());
    let name = name.split_once("::").unwrap().1;
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(CHILD, "1")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("running 1 test"), "{stdout}");
    assert!(
        output.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
