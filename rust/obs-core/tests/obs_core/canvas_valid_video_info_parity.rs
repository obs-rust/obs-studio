//! Tier 3: the original extracted `obs_canvas_has_valid_video_info()` (over
//! the original C canvas rules) versus the shim, on one real-header
//! `struct obs_canvas` filled with any byte, through the real-named `ovi`
//! adapter (oracle/canvas_valid_video_info.c). Compared: the result, the
//! whole canvas's bytes and the allocation count. The real-header
//! `obs_video_info` offsets the shim reads are checked against
//! [`VideoInfo`].
//!
//! Excluded: a NULL canvas (a NULL dereference in both; Tier 2's
//! characterization leaves it untested, as the C does). No intentional
//! differences. Mutants show the probe catches each broken conjunct and an
//! allocation. The allocation count is process-wide (`bnum_allocs`), so the
//! probing tests re-exec in isolation (#5382).
use core::cell::RefCell;
use core::ffi::{c_int, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_core::ffi::canvas_valid_video_info::obs_canvas_has_valid_video_info;
use obs_core::ffi::canvas_video_info_getter::VideoInfo;
use obs_util::ffi::darray::{bfree, bmalloc};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};

type Check = unsafe extern "C" fn(*const c_void) -> bool;
unsafe extern "C" {
    fn oracle_canvas_valid_video_info_layout(out: *mut usize);
    fn oracle_canvas_valid_video_info_adapter() -> bool;
    fn oracle_canvas_valid_video_info_probe(
        candidate: Check,
        fill: u8,
        base_width: u32,
        base_height: u32,
        output_width: u32,
        output_height: u32,
        output_format: c_int,
    ) -> u32;
    fn obs_canvas_has_valid_video_info_ovi(canvas: *const c_void) -> *const VideoInfo;
}

fn probe(candidate: Check, fill: u8, sizes: [u32; 4], format: c_int) -> u32 {
    let [bw, bh, ow, oh] = sizes;
    // SAFETY: the oracle owns the real-layout canvas and passes the
    // candidate only that live canvas.
    unsafe { oracle_canvas_valid_video_info_probe(candidate, fill, bw, bh, ow, oh, format) }
}

/// Whether this process runs `test`'s body: true in the re-exec'd child,
/// which runs only that test. The parent runs it there, checks it passed and
/// returns false. Other tests calling `bmalloc`/`bfree` on other threads would
/// otherwise move the probe's allocation count.
fn isolated(test: &str) -> bool {
    const CHILD: &str = "OBS_CANVAS_VALID_VIDEO_INFO_PARITY_CHILD";
    if std::env::var_os(CHILD).is_some() {
        return true;
    }
    let name = format!("{}::{test}", module_path!().split_once("::").unwrap().1);
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &name, "--nocapture"])
        .env(CHILD, "1")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("1 passed"), "{stdout}");
    false
}

#[test]
fn real_header_layout() {
    let mut out = [0; 8];
    // SAFETY: the oracle writes eight size_t values.
    unsafe { oracle_canvas_valid_video_info_layout(out.as_mut_ptr()) };
    assert_eq!(
        out,
        [
            size_of::<VideoInfo>(),
            align_of::<VideoInfo>(),
            offset_of!(VideoInfo, base_width),
            offset_of!(VideoInfo, base_height),
            offset_of!(VideoInfo, output_width),
            offset_of!(VideoInfo, output_height),
            offset_of!(VideoInfo, output_format),
            size_of::<c_int>(),
        ]
    );
    // SAFETY: the oracle passes a local canvas to the real adapter.
    assert!(unsafe { oracle_canvas_valid_video_info_adapter() });
}

const SIZES: [u32; 5] = [0, 1, 2, u32::MAX / 2, u32::MAX];
const FORMATS: [c_int; 7] = [0, 1, 6, c_int::MIN, -1, c_int::MAX, 2];

#[test]
fn characterized_edges() {
    if !isolated("characterized_edges") {
        return;
    }
    for fill in [0x00, 0xa5, 0xff] {
        for bw in SIZES {
            for bh in SIZES {
                for ow in SIZES {
                    for oh in SIZES {
                        for format in FORMATS {
                            let diff = probe(
                                obs_canvas_has_valid_video_info,
                                fill,
                                [bw, bh, ow, oh],
                                format,
                            );
                            assert_eq!(diff, 0, "{fill:#x} {bw} {bh} {ow} {oh} {format}");
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn any_canvas_matches_c() {
    if !isolated("any_canvas_matches_c") {
        return;
    }
    let config = Config {
        cases: obs_c_oracle::parity::cases(1024),
        failure_persistence: None,
        ..Config::default()
    };
    let strategy = (
        any::<u8>(),
        prop::array::uniform4(prop_oneof![Just(0u32), any::<u32>()]),
        prop_oneof![Just(0), any::<c_int>()],
    );
    TestRunner::new(config)
        .run(&strategy, |(fill, sizes, format)| {
            prop_assert_eq!(
                probe(obs_canvas_has_valid_video_info, fill, sizes, format),
                0
            );
            Ok(())
        })
        .unwrap();
}

fn read(canvas: *const c_void) -> VideoInfo {
    // SAFETY: mutants are called only by the probe with its live canvas.
    unsafe { core::ptr::read(obs_canvas_has_valid_video_info_ovi(canvas)) }
}

unsafe extern "C" fn skips_output_height(canvas: *const c_void) -> bool {
    let v = read(canvas);
    v.base_width != 0 && v.base_height != 0 && v.output_width != 0 && v.output_format != 0
}
unsafe extern "C" fn skips_format(canvas: *const c_void) -> bool {
    let v = read(canvas);
    v.base_width != 0 && v.base_height != 0 && v.output_width != 0 && v.output_height != 0
}
unsafe extern "C" fn positive_format_only(canvas: *const c_void) -> bool {
    let v = read(canvas);
    v.base_width != 0
        && v.base_height != 0
        && v.output_width != 0
        && v.output_height != 0
        && v.output_format > 0
}
unsafe extern "C" fn any_conjunct(canvas: *const c_void) -> bool {
    let v = read(canvas);
    v.base_width != 0
        || v.base_height != 0
        || v.output_width != 0
        || v.output_height != 0
        || v.output_format != 0
}
unsafe extern "C" fn bounded_sizes(canvas: *const c_void) -> bool {
    let v = read(canvas);
    [v.base_width, v.base_height, v.output_width, v.output_height]
        .iter()
        .all(|&s| s != 0 && s < u32::MAX)
        && v.output_format != 0
}
unsafe extern "C" fn clears_format(canvas: *const c_void) -> bool {
    // SAFETY: as in `read`; writes the live canvas's own field.
    let ovi = unsafe { obs_canvas_has_valid_video_info_ovi(canvas) }.cast_mut();
    // SAFETY: as above.
    let valid = unsafe { obs_canvas_has_valid_video_info(canvas) };
    // SAFETY: as above.
    unsafe { (*ovi).output_format = 0 };
    valid
}

thread_local! {
    static HELD: RefCell<Vec<*mut c_void>> = const { RefCell::new(Vec::new()) };
}

unsafe extern "C" fn allocates(canvas: *const c_void) -> bool {
    // SAFETY: a plain allocation, freed by `mutants_are_caught`.
    let block = unsafe { bmalloc(1) };
    HELD.with_borrow_mut(|held| held.push(block));
    // SAFETY: as in `read`.
    unsafe { obs_canvas_has_valid_video_info(canvas) }
}

#[test]
fn mutants_are_caught() {
    if !isolated("mutants_are_caught") {
        return;
    }
    let caught = |candidate: Check| {
        let mut diff = 0;
        for bw in SIZES {
            for oh in SIZES {
                for format in FORMATS {
                    diff |= probe(candidate, 0xa5, [bw, 1, 1, oh], format);
                }
            }
        }
        diff
    };
    // The real shim passes the same sweep.
    assert_eq!(caught(obs_canvas_has_valid_video_info), 0);
    for (name, mutant, bit) in [
        ("skips_output_height", skips_output_height as Check, 1),
        ("skips_format", skips_format, 1),
        ("positive_format_only", positive_format_only, 1),
        ("any_conjunct", any_conjunct, 1),
        ("bounded_sizes", bounded_sizes, 1),
        ("clears_format", clears_format, 4),
        ("allocates", allocates, 8),
    ] {
        assert_ne!(caught(mutant) & bit, 0, "{name} survived");
    }
    for block in HELD.take() {
        // SAFETY: each block came from `bmalloc` in `allocates`.
        unsafe { bfree(block) };
    }
}
