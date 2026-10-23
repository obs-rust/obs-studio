//! Tier 3: the `gs_texrender_*` shim against the original
//! `texture-render.c.h`, compiled as `oracle_gs_texrender_*`.
//!
//! Both sides call the oracle crate's recording `gs_*` stand-in
//! (oracle/texture_render_device.c), which logs every call with its
//! arguments, null destroys and matrix calls included. A case runs one
//! sequence of operations on each side from the same device state and
//! compares the full call log, every return value, the device handles they
//! return and `bnum_allocs` after each step. Null handles are passed to every
//! function the C checks for null.
//!
//! No intentional differences. Not called, because the C does not check:
//! `gs_texrender_get_format(NULL)`.

use core::ffi::{CStr, c_int, c_void};

use obs_c_oracle::texture_render as c;
use obs_graphics::ffi::texture_render as r;
use obs_graphics::texture_render::{ColorFormat, ColorSpace, ZStencilFormat};
use obs_util::ffi::bmem::bnum_allocs;
use proptest::prelude::*;

#[derive(Clone, Copy, Debug)]
enum Op {
    Create {
        format: u32,
        zsformat: u32,
    },
    Destroy(usize),
    Begin {
        slot: usize,
        cx: u32,
        cy: u32,
    },
    BeginWithColorSpace {
        slot: usize,
        cx: u32,
        cy: u32,
        space: u32,
    },
    End(usize),
    Reset(usize),
    GetTexture(usize),
    GetFormat(usize),
    Fail {
        texture: bool,
        zstencil: bool,
    },
    SetState {
        target: c_int,
        zstencil: c_int,
        space: c_int,
    },
}

/// `slot == NULL_SLOT` passes a null texrender.
const NULL_SLOT: usize = usize::MAX;

trait Side {
    /// # Safety
    /// The C function's own contract, with a null or live handle.
    unsafe fn create(format: u32, zsformat: u32) -> *mut c_void;
    unsafe fn destroy(t: *mut c_void);
    unsafe fn begin(t: *mut c_void, cx: u32, cy: u32) -> bool;
    unsafe fn begin_with_color_space(t: *mut c_void, cx: u32, cy: u32, space: u32) -> bool;
    unsafe fn end(t: *mut c_void);
    unsafe fn reset(t: *mut c_void);
    unsafe fn get_texture(t: *mut c_void) -> *mut c_void;
    unsafe fn get_format(t: *mut c_void) -> u32;
}

struct Rust;
struct Original;

impl Side for Rust {
    unsafe fn create(format: u32, zsformat: u32) -> *mut c_void {
        // SAFETY: plain values.
        unsafe { r::gs_texrender_create(ColorFormat(format), ZStencilFormat(zsformat)) }.cast()
    }
    unsafe fn destroy(t: *mut c_void) {
        // SAFETY: null or live, per the trait contract.
        unsafe { r::gs_texrender_destroy(t.cast()) }
    }
    unsafe fn begin(t: *mut c_void, cx: u32, cy: u32) -> bool {
        // SAFETY: null or live, per the trait contract.
        unsafe { r::gs_texrender_begin(t.cast(), cx, cy) }
    }
    unsafe fn begin_with_color_space(t: *mut c_void, cx: u32, cy: u32, space: u32) -> bool {
        // SAFETY: null or live, per the trait contract.
        unsafe { r::gs_texrender_begin_with_color_space(t.cast(), cx, cy, ColorSpace(space)) }
    }
    unsafe fn end(t: *mut c_void) {
        // SAFETY: null or live, per the trait contract.
        unsafe { r::gs_texrender_end(t.cast()) }
    }
    unsafe fn reset(t: *mut c_void) {
        // SAFETY: null or live, per the trait contract.
        unsafe { r::gs_texrender_reset(t.cast()) }
    }
    unsafe fn get_texture(t: *mut c_void) -> *mut c_void {
        // SAFETY: null or live, per the trait contract.
        unsafe { r::gs_texrender_get_texture(t.cast()) }
    }
    unsafe fn get_format(t: *mut c_void) -> u32 {
        // SAFETY: live, per the trait contract.
        unsafe { r::gs_texrender_get_format(t.cast()) }.0
    }
}

impl Side for Original {
    unsafe fn create(format: u32, zsformat: u32) -> *mut c_void {
        // SAFETY: plain values.
        unsafe { c::oracle_gs_texrender_create(format, zsformat) }
    }
    unsafe fn destroy(t: *mut c_void) {
        // SAFETY: null or live, per the trait contract.
        unsafe { c::oracle_gs_texrender_destroy(t) }
    }
    unsafe fn begin(t: *mut c_void, cx: u32, cy: u32) -> bool {
        // SAFETY: null or live, per the trait contract.
        unsafe { c::oracle_gs_texrender_begin(t, cx, cy) }
    }
    unsafe fn begin_with_color_space(t: *mut c_void, cx: u32, cy: u32, space: u32) -> bool {
        // SAFETY: null or live, per the trait contract.
        unsafe { c::oracle_gs_texrender_begin_with_color_space(t, cx, cy, space) }
    }
    unsafe fn end(t: *mut c_void) {
        // SAFETY: null or live, per the trait contract.
        unsafe { c::oracle_gs_texrender_end(t) }
    }
    unsafe fn reset(t: *mut c_void) {
        // SAFETY: null or live, per the trait contract.
        unsafe { c::oracle_gs_texrender_reset(t) }
    }
    unsafe fn get_texture(t: *mut c_void) -> *mut c_void {
        // SAFETY: null or live, per the trait contract.
        unsafe { c::oracle_gs_texrender_get_texture(t) }
    }
    unsafe fn get_format(t: *mut c_void) -> u32 {
        // SAFETY: live, per the trait contract.
        unsafe { c::oracle_gs_texrender_get_format(t) }
    }
}

/// Everything a run can observe: the device log, then one line per step.
fn run<S: Side>(ops: &[Op]) -> Vec<String> {
    let mut live: Vec<*mut c_void> = Vec::new();
    let mut out = Vec::new();
    // SAFETY: resets the stand-in's process-wide state. Only this test
    // touches it, in its own child process.
    unsafe { c::oracle_texrender_device_reset() };
    // SAFETY: reads the process-wide counter.
    let base = unsafe { bnum_allocs() };
    let pick = |live: &Vec<*mut c_void>, slot: usize| {
        if slot == NULL_SLOT || live.is_empty() {
            core::ptr::null_mut()
        } else {
            live[slot % live.len()]
        }
    };
    for op in ops {
        // SAFETY: each handle is null or one this run created and has not
        // destroyed; get_format is only called with a live one.
        let step = unsafe {
            match *op {
                Op::Create { format, zsformat } => {
                    let t = S::create(format, zsformat);
                    assert!(!t.is_null());
                    live.push(t);
                    "create".to_string()
                }
                Op::Destroy(slot) => {
                    let t = pick(&live, slot);
                    S::destroy(t);
                    live.retain(|&x| x != t);
                    format!("destroy {}", !t.is_null())
                }
                Op::Begin { slot, cx, cy } => {
                    format!("begin {}", S::begin(pick(&live, slot), cx, cy))
                }
                Op::BeginWithColorSpace {
                    slot,
                    cx,
                    cy,
                    space,
                } => format!(
                    "begin_cs {}",
                    S::begin_with_color_space(pick(&live, slot), cx, cy, space)
                ),
                Op::End(slot) => {
                    S::end(pick(&live, slot));
                    "end".to_string()
                }
                Op::Reset(slot) => {
                    S::reset(pick(&live, slot));
                    "reset".to_string()
                }
                Op::GetTexture(slot) => {
                    let tex = S::get_texture(pick(&live, slot));
                    format!("texture {}", c::oracle_texrender_device_handle(tex))
                }
                Op::GetFormat(slot) => {
                    let t = pick(&live, slot);
                    if t.is_null() {
                        "format skipped".to_string()
                    } else {
                        format!("format {}", S::get_format(t))
                    }
                }
                Op::Fail { texture, zstencil } => {
                    c::oracle_texrender_device_fail(texture, zstencil);
                    "fail".to_string()
                }
                Op::SetState {
                    target,
                    zstencil,
                    space,
                } => {
                    c::oracle_texrender_device_set_state(target, zstencil, space);
                    "state".to_string()
                }
            }
        };
        // SAFETY: reads the process-wide counter.
        let allocs = unsafe { bnum_allocs() } - base;
        out.push(format!("{step} allocs={allocs}"));
    }
    for t in live.drain(..) {
        // SAFETY: each is live and destroyed once.
        unsafe { S::destroy(t) };
    }
    // SAFETY: reads the process-wide counter.
    out.push(format!("end allocs={}", unsafe { bnum_allocs() } - base));
    // SAFETY: the stand-in's log is a NUL-terminated static buffer.
    let log = unsafe { CStr::from_ptr(c::oracle_texrender_device_log()) };
    out.insert(0, log.to_string_lossy().into_owned());
    out
}

fn compare(ops: &[Op]) {
    let original = run::<Original>(ops);
    let rust = run::<Rust>(ops);
    assert_eq!(rust, original, "ops: {ops:?}");
    assert_eq!(original.last().unwrap(), "end allocs=0", "ops: {ops:?}");
}

/// The cmocka cases' situations, as fixed sequences.
fn scenarios() -> Vec<Vec<Op>> {
    use Op::*;
    let fmt = |format, zsformat| Create { format, zsformat };
    vec![
        // Null handle to every function that checks it.
        vec![
            Destroy(NULL_SLOT),
            Begin {
                slot: NULL_SLOT,
                cx: 64,
                cy: 32,
            },
            BeginWithColorSpace {
                slot: NULL_SLOT,
                cx: 64,
                cy: 32,
                space: 3,
            },
            End(NULL_SLOT),
            Reset(NULL_SLOT),
            GetTexture(NULL_SLOT),
        ],
        // Create and destroy without begin; begin, end, rendered, reset.
        vec![
            fmt(3, 0),
            GetTexture(0),
            GetFormat(0),
            Destroy(0),
            fmt(6, 2),
            Begin {
                slot: 0,
                cx: 64,
                cy: 32,
            },
            GetTexture(0),
            End(0),
            Begin {
                slot: 0,
                cx: 64,
                cy: 32,
            },
            Begin {
                slot: 0,
                cx: 128,
                cy: 64,
            },
            Reset(0),
            Reset(0),
            Begin {
                slot: 0,
                cx: 64,
                cy: 32,
            },
            End(0),
        ],
        // Resize, zero sizes and the int cast of the viewport size.
        vec![
            fmt(9, 3),
            Begin {
                slot: 0,
                cx: 0,
                cy: 32,
            },
            Begin {
                slot: 0,
                cx: 32,
                cy: 0,
            },
            Begin {
                slot: 0,
                cx: 64,
                cy: 32,
            },
            End(0),
            Reset(0),
            Begin {
                slot: 0,
                cx: 64,
                cy: 33,
            },
            End(0),
            Reset(0),
            Begin {
                slot: 0,
                cx: 0x8000_0000,
                cy: 0xffff_ffff,
            },
            End(0),
        ],
        // Texture create failure, then the same size and another size.
        vec![
            fmt(3, 1),
            Fail {
                texture: true,
                zstencil: false,
            },
            Begin {
                slot: 0,
                cx: 64,
                cy: 32,
            },
            Fail {
                texture: false,
                zstencil: false,
            },
            Begin {
                slot: 0,
                cx: 64,
                cy: 32,
            },
            Begin {
                slot: 0,
                cx: 64,
                cy: 31,
            },
            End(0),
        ],
        // Zstencil create failure.
        vec![
            fmt(3, 2),
            Fail {
                texture: false,
                zstencil: true,
            },
            Begin {
                slot: 0,
                cx: 64,
                cy: 32,
            },
            GetTexture(0),
            Fail {
                texture: false,
                zstencil: false,
            },
            Begin {
                slot: 0,
                cx: 64,
                cy: 32,
            },
            Destroy(0),
        ],
        // Color spaces and a null current target.
        vec![
            fmt(9, 0),
            SetState {
                target: -1,
                zstencil: -1,
                space: 1,
            },
            BeginWithColorSpace {
                slot: 0,
                cx: 10,
                cy: 20,
                space: 3,
            },
            End(0),
            Reset(0),
            Begin {
                slot: 0,
                cx: 10,
                cy: 20,
            },
            End(0),
        ],
        // Double begin, double end, end without begin.
        vec![
            fmt(3, 0),
            Begin {
                slot: 0,
                cx: 64,
                cy: 32,
            },
            Begin {
                slot: 0,
                cx: 64,
                cy: 32,
            },
            End(0),
            End(0),
            fmt(3, 0),
            End(1),
            Begin {
                slot: 1,
                cx: 64,
                cy: 32,
            },
            Reset(1),
            Begin {
                slot: 1,
                cx: 64,
                cy: 32,
            },
            End(1),
            End(1),
        ],
    ]
}

fn size() -> impl Strategy<Value = u32> {
    prop_oneof![4 => 0u32..4, 4 => Just(64u32), 1 => any::<u32>()]
}

fn slot() -> impl Strategy<Value = usize> {
    prop_oneof![8 => 0usize..4, 1 => Just(NULL_SLOT)]
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        2 => (0u32..12, 0u32..5).prop_map(|(format, zsformat)| Op::Create { format, zsformat }),
        1 => slot().prop_map(Op::Destroy),
        4 => (slot(), size(), size()).prop_map(|(slot, cx, cy)| Op::Begin { slot, cx, cy }),
        2 => (slot(), size(), size(), 0u32..4).prop_map(|(slot, cx, cy, space)| {
            Op::BeginWithColorSpace { slot, cx, cy, space }
        }),
        4 => slot().prop_map(Op::End),
        3 => slot().prop_map(Op::Reset),
        1 => slot().prop_map(Op::GetTexture),
        1 => slot().prop_map(Op::GetFormat),
        1 => (any::<bool>(), any::<bool>())
            .prop_map(|(texture, zstencil)| Op::Fail { texture, zstencil }),
        1 => (-1i32..3, -1i32..3, 0i32..4).prop_map(|(target, zstencil, space)| Op::SetState {
            target,
            zstencil,
            space
        }),
    ]
}

#[test]
fn texrender_matches_original_c() {
    const CHILD: &str = "OBS_TEXRENDER_PARITY_CHILD";
    if std::env::var_os(CHILD).is_none() {
        // The stand-in device and allocation counter are process-wide:
        // re-exec this exact case alone, in this same test executable.
        let name = format!("{}::texrender_matches_original_c", module_path!());
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
    for ops in scenarios() {
        compare(&ops);
    }
    proptest::test_runner::TestRunner::new(ProptestConfig::with_cases(512))
        .run(&proptest::collection::vec(op(), 0..48), |ops| {
            compare(&ops);
            Ok(())
        })
        .unwrap();
}
