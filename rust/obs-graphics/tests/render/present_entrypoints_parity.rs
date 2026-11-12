//! Tier 3: the Rust frame and present calls against the original C
//! (`present-entrypoints.c.h`, compiled as `oracle_gs_*` with its own copy
//! of `gs_valid`) over the real private graphics layout, a stand-in context,
//! a recording device and a recording `blog`. Both sides see the same
//! context, device and answer; the test compares the device calls (with
//! their device, arguments and pointer identities), the results and the log
//! lines, call by call. Inputs: random draw modes (outside the enum too),
//! vertex ranges, clear flags, depth bits (NaN payloads, signed zero,
//! infinities), stencils, colours and swapchains (NULL too), both devices,
//! both present-ready answers, and no context. No intentional differences.
use obs_c_oracle as _;
use obs_graphics::ffi::present_entrypoints as rust;
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
struct Event {
    call: c_int,
    device: *const c_void,
    draw_mode: c_int,
    start_vert: u32,
    num_verts: u32,
    pointer: *const c_void,
    clear_flags: u32,
    depth_bits: u32,
    stencil: u8,
}

const NO_EVENT: Event = Event {
    call: -1,
    device: ptr::null(),
    draw_mode: 0,
    start_vert: 0,
    num_verts: 0,
    pointer: ptr::null(),
    clear_flags: 0,
    depth_bits: 0,
    stencil: 0,
};

unsafe extern "C" {
    fn oracle_gs_begin_frame();
    fn oracle_gs_begin_scene();
    fn oracle_gs_draw(mode: c_int, start: u32, count: u32);
    fn oracle_gs_end_scene();
    fn oracle_gs_load_swapchain(swapchain: *mut c_void);
    fn oracle_gs_clear(flags: u32, color: *const c_void, depth: f32, stencil: u8);
    fn oracle_gs_is_present_ready() -> bool;
    fn oracle_gs_present();
    fn oracle_gs_flush();
    fn oracle_present_reset(context: bool, device: *mut c_void, present_ready: bool);
    fn oracle_present_events(out: *mut Event, max: usize) -> usize;
    fn oracle_present_logs() -> u32;
    fn oracle_present_level() -> c_int;
    fn oracle_present_log() -> *const c_char;
}

static DEVICES: [u8; 2] = [0; 2];
static SWAPCHAINS: [u8; 2] = [0; 2];
static COLORS: [[f32; 4]; 2] = [[0.25, -0.5, 1.0, 0.0], [1.0; 4]];

fn device(index: usize) -> *mut c_void {
    ptr::from_ref(&DEVICES[index]).cast_mut().cast()
}

fn swapchain(index: Option<usize>) -> *mut c_void {
    index.map_or(ptr::null_mut(), |i| {
        ptr::from_ref(&SWAPCHAINS[i]).cast_mut().cast()
    })
}

fn color(index: Option<usize>) -> *const c_void {
    index.map_or(ptr::null(), |i| ptr::from_ref(&COLORS[i]).cast())
}

#[derive(Clone, Copy, Debug)]
enum Op {
    BeginFrame,
    BeginScene,
    Draw(i32, u32, u32),
    EndScene,
    LoadSwapchain(Option<usize>),
    Clear(u32, Option<usize>, u32, u8),
    IsPresentReady,
    Present,
    Flush,
}

#[derive(Debug, PartialEq)]
struct Observed {
    ready: Option<bool>,
    events: Vec<Event>,
    logs: u32,
    level: c_int,
    log: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
struct Step {
    context: bool,
    device: usize,
    present_ready: bool,
    op: Op,
}

fn run(use_rust: bool, step: Step) -> Observed {
    // SAFETY: the stand-in context is this thread's and is reset before the
    // call; every pointer passed is a static the device only records.
    unsafe {
        oracle_present_reset(step.context, device(step.device), step.present_ready);
        let mut ready = None;
        match (use_rust, step.op) {
            (true, Op::BeginFrame) => rust::gs_begin_frame(),
            (false, Op::BeginFrame) => oracle_gs_begin_frame(),
            (true, Op::BeginScene) => rust::gs_begin_scene(),
            (false, Op::BeginScene) => oracle_gs_begin_scene(),
            (true, Op::Draw(m, s, n)) => rust::gs_draw(m, s, n),
            (false, Op::Draw(m, s, n)) => oracle_gs_draw(m, s, n),
            (true, Op::EndScene) => rust::gs_end_scene(),
            (false, Op::EndScene) => oracle_gs_end_scene(),
            (true, Op::LoadSwapchain(s)) => rust::gs_load_swapchain(swapchain(s)),
            (false, Op::LoadSwapchain(s)) => oracle_gs_load_swapchain(swapchain(s)),
            (true, Op::Clear(f, c, d, s)) => rust::gs_clear(f, color(c), f32::from_bits(d), s),
            (false, Op::Clear(f, c, d, s)) => oracle_gs_clear(f, color(c), f32::from_bits(d), s),
            (true, Op::IsPresentReady) => ready = Some(rust::gs_is_present_ready()),
            (false, Op::IsPresentReady) => ready = Some(oracle_gs_is_present_ready()),
            (true, Op::Present) => rust::gs_present(),
            (false, Op::Present) => oracle_gs_present(),
            (true, Op::Flush) => rust::gs_flush(),
            (false, Op::Flush) => oracle_gs_flush(),
        }
        let mut events = [NO_EVENT; 8];
        let n = oracle_present_events(events.as_mut_ptr(), events.len());
        Observed {
            ready,
            events: events[..n].to_vec(),
            logs: oracle_present_logs(),
            level: oracle_present_level(),
            log: CStr::from_ptr(oracle_present_log()).to_bytes().to_vec(),
        }
    }
}

fn name(op: Op) -> &'static str {
    match op {
        Op::BeginFrame => "gs_begin_frame",
        Op::BeginScene => "gs_begin_scene",
        Op::Draw(..) => "gs_draw",
        Op::EndScene => "gs_end_scene",
        Op::LoadSwapchain(_) => "gs_load_swapchain",
        Op::Clear(..) => "gs_clear",
        Op::IsPresentReady => "gs_is_present_ready",
        Op::Present => "gs_present",
        Op::Flush => "gs_flush",
    }
}

fn compare(step: Step) {
    let original = run(false, step);
    let ported = run(true, step);
    assert_eq!(ported, original, "{step:?}");
    // And what the C does, so a fault common to both sides shows too.
    if step.context {
        assert_eq!(ported.logs, 0);
        assert_eq!(ported.events.len(), 1);
        assert_eq!(ported.events[0].device, device(step.device).cast_const());
    } else {
        assert!(ported.events.is_empty());
        assert_eq!((ported.logs, ported.level), (1, 400));
        let line = format!("{}: called while not in a graphics context", name(step.op));
        assert_eq!(ported.log, line.as_bytes());
    }
    if let Op::IsPresentReady = step.op {
        assert_eq!(ported.ready, Some(step.context && step.present_ready));
    }
}

fn edge_ops() -> Vec<Op> {
    let mut ops = vec![
        Op::BeginFrame,
        Op::BeginScene,
        Op::EndScene,
        Op::IsPresentReady,
        Op::Present,
        Op::Flush,
    ];
    for s in [Some(0), Some(1), None] {
        ops.push(Op::LoadSwapchain(s));
    }
    for mode in [0, 1, 2, 3, 4, 5, 99, -1, i32::MIN, i32::MAX] {
        for (start, count) in [(0, 0), (0, 3), (7, 1), (u32::MAX, u32::MAX), (1, u32::MAX)] {
            ops.push(Op::Draw(mode, start, count));
        }
    }
    let flags = [0, 1, 2, 4, 7, 0xFFFF_FFF8, u32::MAX];
    let depths = [
        0,
        0x8000_0000,
        1.0f32.to_bits(),
        (-2.5f32).to_bits(),
        0x7F80_0000,
        0xFF80_0000,
        0x7FC0_0001,
        0x7F80_0001,
        0x0000_0001,
    ];
    let mut n = 0usize;
    for flag in flags {
        for depth in depths {
            let c = [Some(0), None, Some(1)][n % 3];
            let stencil = [0, 1, 0x80, 0xFF][n % 4];
            n += 1;
            ops.push(Op::Clear(flag, c, depth, stencil));
        }
    }
    ops
}

#[test]
fn characterized_matrix_matches_c() {
    for op in edge_ops() {
        for context in [false, true] {
            for device in 0..2 {
                for present_ready in [false, true] {
                    compare(Step {
                        context,
                        device,
                        present_ready,
                        op,
                    });
                }
            }
        }
    }
}

fn index() -> impl Strategy<Value = Option<usize>> {
    prop::option::of(0usize..2)
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        Just(Op::BeginFrame),
        Just(Op::BeginScene),
        (
            prop_oneof![0i32..=4, any::<i32>()],
            any::<u32>(),
            any::<u32>()
        )
            .prop_map(|(m, s, n)| Op::Draw(m, s, n)),
        Just(Op::EndScene),
        index().prop_map(Op::LoadSwapchain),
        (any::<u32>(), index(), any::<u32>(), any::<u8>())
            .prop_map(|(f, c, d, s)| Op::Clear(f, c, d, s)),
        Just(Op::IsPresentReady),
        Just(Op::Present),
        Just(Op::Flush),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default()
    })]
    #[test]
    fn random_sequences_match_c(
        steps in prop::collection::vec(
            (any::<bool>(), 0usize..2, any::<bool>(), op()),
            1..=16,
        ),
    ) {
        for (context, device, present_ready, op) in steps {
            compare(Step { context, device, present_ready, op });
        }
    }
}
