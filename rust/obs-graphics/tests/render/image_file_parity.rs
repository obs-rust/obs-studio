//! Tier 3: the `gs_image_file_ex_*` shim against the original
//! `image-file.c.h`, compiled as `oracle_gs_image_file_ex_*`.
//!
//! Both sides read the real fixture files (`test/cmocka/data/image-file-*`)
//! through the same libnsgif and call the oracle crate's recording `gs_*`
//! stand-in (oracle/texture_render_device.c), which logs every texture call
//! with a hash of the pixels it is given, the FFmpeg stand-in's calls and
//! every libobs log line. A case runs one sequence of operations (init,
//! texture init, ticks, texture updates, free) on each side from the same
//! device state and compares that log, every return value, every public
//! field of `struct gs_image_file_ex` and `bnum_allocs` after each step.
//!
//! No intentional differences.

use core::ffi::{CStr, c_void};
use core::mem::MaybeUninit;
use std::ffi::CString;

use obs_c_oracle::image_file as c;
use obs_c_oracle::texture_render as device;
use obs_graphics::ffi::image_file::{self as r, gs_image_file_ex_t};
use obs_graphics::image_file::ImageAlphaMode;
use obs_util::ffi::bmem::bnum_allocs;
use proptest::prelude::*;

const MS: u64 = 1_000_000;

const FILES: [&str; 8] = [
    "image-file-anim.gif",
    "image-file-loop2.gif",
    "image-file-noloop.gif",
    "image-file-still.gif",
    "image-file-wide.gif",
    "image-file.png",
    "image-file-missing.gif",
    "image-file-missing.png",
];

fn fixture(name: &str) -> CString {
    CString::new(format!(
        "{}/../../test/cmocka/data/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

#[derive(Clone, Copy, Debug)]
enum Op {
    InitTexture,
    Tick(u64),
    Update,
}

trait Side {
    /// # Safety
    /// The C function's own contract.
    unsafe fn init(image: *mut gs_image_file_ex_t, file: *const core::ffi::c_char, alpha: u32);
    unsafe fn free(image: *mut gs_image_file_ex_t);
    unsafe fn init_texture(image: *mut gs_image_file_ex_t);
    unsafe fn tick(image: *mut gs_image_file_ex_t, elapsed: u64) -> bool;
    unsafe fn update_texture(image: *mut gs_image_file_ex_t);
}

struct Rust;
struct Original;

impl Side for Rust {
    unsafe fn init(image: *mut gs_image_file_ex_t, file: *const core::ffi::c_char, alpha: u32) {
        // SAFETY: per the trait contract.
        unsafe { r::gs_image_file_ex_init(image, file, ImageAlphaMode(alpha)) }
    }
    unsafe fn free(image: *mut gs_image_file_ex_t) {
        // SAFETY: per the trait contract.
        unsafe { r::gs_image_file_ex_free(image) }
    }
    unsafe fn init_texture(image: *mut gs_image_file_ex_t) {
        // SAFETY: per the trait contract.
        unsafe { r::gs_image_file_ex_init_texture(image) }
    }
    unsafe fn tick(image: *mut gs_image_file_ex_t, elapsed: u64) -> bool {
        // SAFETY: per the trait contract.
        unsafe { r::gs_image_file_ex_tick(image, elapsed) }
    }
    unsafe fn update_texture(image: *mut gs_image_file_ex_t) {
        // SAFETY: per the trait contract.
        unsafe { r::gs_image_file_ex_update_texture(image) }
    }
}

impl Side for Original {
    unsafe fn init(image: *mut gs_image_file_ex_t, file: *const core::ffi::c_char, alpha: u32) {
        // SAFETY: per the trait contract; the struct layouts match
        // (image_file_layout.rs).
        unsafe { c::oracle_gs_image_file_ex_init(image.cast(), file, alpha) }
    }
    unsafe fn free(image: *mut gs_image_file_ex_t) {
        // SAFETY: per the trait contract.
        unsafe { c::oracle_gs_image_file_ex_free(image.cast()) }
    }
    unsafe fn init_texture(image: *mut gs_image_file_ex_t) {
        // SAFETY: per the trait contract.
        unsafe { c::oracle_gs_image_file_ex_init_texture(image.cast()) }
    }
    unsafe fn tick(image: *mut gs_image_file_ex_t, elapsed: u64) -> bool {
        // SAFETY: per the trait contract.
        unsafe { c::oracle_gs_image_file_ex_tick(image.cast(), elapsed) }
    }
    unsafe fn update_texture(image: *mut gs_image_file_ex_t) {
        // SAFETY: per the trait contract.
        unsafe { c::oracle_gs_image_file_ex_update_texture(image.cast()) }
    }
}

/// Every public field; pointers as the stand-in's handle or null/non-null.
fn fields(image: &gs_image_file_ex_t, base: i64) -> String {
    let texture = image
        .texture
        .map_or(core::ptr::null(), |t| t.as_ptr() as *const c_void);
    // SAFETY: a pointer value, only compared against the stand-in's pools.
    let texture = unsafe { device::oracle_texrender_device_handle(texture) };
    let allocs = allocs_now() - base;
    format!(
        "tex={texture} f{} {}x{} anim={} upd={} loaded={} internal={} t={} frame={} loop={} \
         data={} mem={} alpha={} space={} allocs={allocs}",
        image.format.0,
        image.cx,
        image.cy,
        image.is_animated_gif,
        image.frame_updated,
        image.loaded,
        image.internal.is_some(),
        image.cur_time,
        image.cur_frame,
        image.cur_loop,
        image.texture_data.is_some(),
        image.mem_usage,
        image.alpha_mode.0,
        image.space.0,
    )
}

/// `bnum_allocs`, whose `long` is 32 bits on Windows.
#[allow(clippy::useless_conversion)]
fn allocs_now() -> i64 {
    // SAFETY: reads the process-wide counter.
    i64::from(unsafe { bnum_allocs() })
}

fn take_log() -> String {
    // SAFETY: the stand-in's log is a NUL-terminated static buffer.
    let log = unsafe { CStr::from_ptr(device::oracle_texrender_device_log()) }
        .to_string_lossy()
        .into_owned();
    // SAFETY: clears the stand-in's process-wide log.
    unsafe { device::oracle_texrender_device_clear_log() };
    log
}

/// Everything a run can observe, one line per step.
fn run<S: Side>(file: usize, alpha: u32, ops: &[Op]) -> Vec<String> {
    let path = fixture(FILES[file]);
    let mut out = Vec::new();
    let mut image = MaybeUninit::<gs_image_file_ex_t>::uninit();
    let image = image.as_mut_ptr();
    // SAFETY: resets the stand-in's process-wide state and sends log lines
    // to its log. Only this test touches it, in its own child process.
    unsafe {
        device::oracle_texrender_device_reset();
        device::oracle_texrender_device_capture_log(true);
    }
    let base = allocs_now();
    // SAFETY: init writes the whole struct; every later call gets the
    // initialized image.
    unsafe {
        S::init(image, path.as_ptr(), alpha);
        out.push(format!("init {} | {}", fields(&*image, base), take_log()));
        for op in ops {
            let step = match *op {
                Op::InitTexture => {
                    S::init_texture(image);
                    "init_texture".to_string()
                }
                Op::Tick(elapsed) => format!("tick {elapsed} {}", S::tick(image, elapsed)),
                Op::Update => {
                    S::update_texture(image);
                    "update".to_string()
                }
            };
            out.push(format!("{step} {} | {}", fields(&*image, base), take_log()));
        }
        S::free(image);
        out.push(format!("free {} | {}", fields(&*image, base), take_log()));
        device::oracle_texrender_device_capture_log(false);
    }
    out
}

fn compare(file: usize, alpha: u32, ops: &[Op]) {
    let original = run::<Original>(file, alpha, ops);
    let rust = run::<Rust>(file, alpha, ops);
    assert_eq!(rust, original, "{} alpha {alpha} ops {ops:?}", FILES[file]);
    assert!(
        original.last().unwrap().contains("allocs=0 "),
        "{} leaks: {original:?}",
        FILES[file]
    );
}

fn elapsed() -> impl Strategy<Value = u64> {
    prop_oneof![
        3 => 0..300 * MS,
        2 => prop::sample::select(vec![
            0,
            1,
            50 * MS,
            50 * MS + 1,
            100 * MS - 1,
            100 * MS,
            100 * MS + 1,
            250 * MS,
            250 * MS + 1,
            750 * MS,
        ]),
        1 => 0..2_000 * MS,
    ]
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        1 => Just(Op::InitTexture),
        6 => elapsed().prop_map(Op::Tick),
        3 => Just(Op::Update),
    ]
}

#[test]
fn image_file_matches_original_c() {
    const CHILD: &str = "OBS_IMAGE_FILE_PARITY_CHILD";
    if std::env::var_os(CHILD).is_none() {
        // The stand-in device, log handler and allocation counter are
        // process-wide: re-exec this exact case alone, in this same test
        // executable.
        let name = format!("{}::image_file_matches_original_c", module_path!());
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
        return;
    }
    // Every file and alpha mode through the whole life of an image.
    let walk = [
        Op::InitTexture,
        Op::Update,
        Op::Tick(50 * MS),
        Op::Tick(50 * MS + 1),
        Op::Update,
        Op::Tick(200 * MS),
        Op::Update,
        Op::Tick(2_000 * MS),
        Op::Update,
        Op::Tick(1),
    ];
    for file in 0..FILES.len() {
        for alpha in 0..3 {
            compare(file, alpha, &[]);
            compare(file, alpha, &walk);
        }
    }
    proptest::test_runner::TestRunner::new(ProptestConfig::with_cases(
        obs_c_oracle::parity::cases(512),
    ))
    .run(
        &(
            0..FILES.len(),
            0u32..3,
            proptest::collection::vec(op(), 0..40),
        ),
        |(file, alpha, ops)| {
            compare(file, alpha, &ops);
            Ok(())
        },
    )
    .unwrap();
}
