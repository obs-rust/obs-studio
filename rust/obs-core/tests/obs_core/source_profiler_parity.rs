//! Tier 2 layout and Tier 3 parity: `obs_core::source_profiler` against the
//! original `libobs/util/source-profiler.c.h`.
//!
//! The oracle (`rust/obs-c-oracle/oracle/source_profiler.c`) is the C file,
//! unmodified, with its names renamed and everything it asks libobs for
//! (clock, GPU timers and ranges, graphics context, task queue, async
//! timestamp, log) replaced by deterministic stand-ins. The Rust core runs
//! the same operations with a [`Host`] that answers alike: the same clock
//! readings, the same handle numbers (timers and ranges counted from
//! `0x9000` in steps of `0x10`), the same ticks per timer, the same range
//! data. Sources are the oracle's zeroed `struct obs_source` blocks, so both
//! sides key on the same addresses.
//!
//! Compared after every operation of a random sequence (enable, GPU enable,
//! video reset, whole frames with ticks, render passes, GPU ticks, disjoint
//! ranges and NULL timers, async frames, last rendered timestamps, source
//! removal): the flags, the timer slot and ranges, the ring capacity, every
//! source's samples and every ring of its entry (values, `num`, `idx`,
//! capacity), `fill_result` for every source and an unknown one (result
//! bytes, or untouched / zeroed), which timers and ranges were destroyed,
//! the disjoint warnings, how many clock readings were taken, and that the
//! graphics context is left as entered. Also the ring on its own, and the
//! tick/render/fps math over any values.
//!
//! Excluded inputs, the intentional differences (#1886): a ring capacity of
//! 0 (no video reset yet, which the C divides by zero on) and a GPU
//! frequency of 0 (the C divides by zero). Not compared: the order of timer
//! destruction within one operation and the number of graphics context
//! entries (the port destroys dropped samples' timers in one context, after
//! its sample lock is released).
//!
//! The shim itself runs once at the end of this file against the cargo-test
//! stand-ins of `oracle/source_profiler_host.c` (no graphics).

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::ffi::c_void;
use std::ptr;
use std::sync::Mutex;

use obs_c_oracle::source_profiler::*;
use obs_core::ffi::source_profiler::{
    obs_video_info_head, profiler_result, source_profiler_async_frame_received,
    source_profiler_enable, source_profiler_fill_result, source_profiler_frame_begin,
    source_profiler_frame_collect, source_profiler_get_result, source_profiler_gpu_enable,
    source_profiler_remove_source, source_profiler_render_begin, source_profiler_render_end,
    source_profiler_reset_video, source_profiler_source_render_begin,
    source_profiler_source_render_end, source_profiler_source_tick_end,
    source_profiler_source_tick_start,
};
use obs_core::source_profiler::{
    CircleBuf, Entry, FRAME_BUFFER_SIZE, Fill, Handle, Host, ProfilerResult, RangeData,
    SourceProfiler, calculate_fps, calculate_render, calculate_tick,
};
use proptest::prelude::*;

/// The oracle is one process-wide profiler.
static ORACLE: Mutex<()> = Mutex::new(());

const OBS_SOURCE_VIDEO: u32 = 1;
const OBS_SOURCE_ASYNC_VIDEO: u32 = 5;
const SOURCE_FLAGS: [u32; 3] = [OBS_SOURCE_VIDEO, OBS_SOURCE_ASYNC_VIDEO, OBS_SOURCE_VIDEO];
const RINGS: usize = 7;

unsafe extern "C" {
    fn bfree(ptr: *mut c_void);
}

// ---------------------------------------------------------------------------
// the Rust side's host, answering as the oracle's stand-ins do

struct Fake {
    times: RefCell<VecDeque<u64>>,
    next_handle: Cell<Handle>,
    null_handles: Cell<bool>,
    next_ticks: Cell<Option<u64>>,
    ticks: RefCell<HashMap<Handle, Option<u64>>>,
    range: Cell<RangeData>,
    async_flags: RefCell<HashMap<Handle, bool>>,
    last_async: RefCell<HashMap<Handle, u64>>,
    destroyed_timers: RefCell<Vec<Handle>>,
    destroyed_ranges: RefCell<Vec<Handle>>,
    warnings: Cell<usize>,
    depth: Cell<i64>,
}

impl Fake {
    fn new() -> Self {
        Self {
            times: RefCell::new(VecDeque::new()),
            next_handle: Cell::new(0x9000),
            null_handles: Cell::new(false),
            next_ticks: Cell::new(Some(0)),
            ticks: RefCell::new(HashMap::new()),
            range: Cell::new(RangeData {
                disjoint: false,
                frequency: 1_000_000_000,
            }),
            async_flags: RefCell::new(HashMap::new()),
            last_async: RefCell::new(HashMap::new()),
            destroyed_timers: RefCell::new(Vec::new()),
            destroyed_ranges: RefCell::new(Vec::new()),
            warnings: Cell::new(0),
            depth: Cell::new(0),
        }
    }

    fn handle(&self) -> Handle {
        if self.null_handles.get() {
            return 0;
        }
        let h = self.next_handle.get();
        self.next_handle.set(h + 0x10);
        h
    }
}

impl Host for Fake {
    fn now_ns(&self) -> u64 {
        self.times.borrow_mut().pop_front().unwrap_or(0)
    }
    fn enter_graphics(&self) {
        self.depth.set(self.depth.get() + 1);
    }
    fn leave_graphics(&self) {
        self.depth.set(self.depth.get() - 1);
    }
    fn timer_create(&self) -> Handle {
        let h = self.handle();
        if h != 0 {
            self.ticks.borrow_mut().insert(h, self.next_ticks.get());
        }
        h
    }
    fn timer_begin(&self, _timer: Handle) {}
    fn timer_end(&self, _timer: Handle) {}
    fn timer_get_data(&self, timer: Handle) -> Option<u64> {
        self.ticks.borrow().get(&timer).copied().flatten()
    }
    fn timer_destroy(&self, timer: Handle) {
        self.destroyed_timers.borrow_mut().push(timer);
    }
    fn range_create(&self) -> Handle {
        self.handle()
    }
    fn range_begin(&self, _range: Handle) {}
    fn range_end(&self, _range: Handle) {}
    fn range_get_data(&self, _range: Handle) -> RangeData {
        self.range.get()
    }
    fn range_destroy(&self, range: Handle) {
        self.destroyed_ranges.borrow_mut().push(range);
    }
    fn is_async_video(&self, source: Handle) -> bool {
        self.async_flags
            .borrow()
            .get(&source)
            .copied()
            .unwrap_or(false)
    }
    fn last_async_ts(&self, source: Handle) -> u64 {
        self.last_async.borrow().get(&source).copied().unwrap_or(0)
    }
    fn warn_disjoint(&self) {
        self.warnings.set(self.warnings.get() + 1);
    }
    fn collect_scope_begin(&self) {}
    fn collect_scope_end(&self) {}
}

// ---------------------------------------------------------------------------
// operations

#[derive(Debug, Clone)]
struct FrameOp {
    ticks: [Option<u64>; 3],
    renders: [Vec<u64>; 3],
    gpu_ticks: Option<u64>,
    disjoint: bool,
    frequency: u64,
    null_handles: bool,
}

#[derive(Debug, Clone)]
enum Op {
    Enable(bool),
    GpuEnable(bool),
    Reset(u32, u32),
    Frame(FrameOp),
    Async(usize, u64),
    LastAsync(usize, u64),
    Remove(usize),
}

fn frame_op() -> impl Strategy<Value = FrameOp> {
    let tick = prop::option::weighted(0.8, 0u64..1_000_000);
    let renders = prop::collection::vec(0u64..1_000_000, 0..3);
    (
        [tick.clone(), tick.clone(), tick],
        [renders.clone(), renders.clone(), renders],
        prop::option::weighted(0.8, 0u64..1_000_000),
        prop::bool::weighted(0.15),
        // frequency 0 is excluded: the C divides by zero
        prop_oneof![Just(1_000_000_000u64), 1u64..=1_000_000_000],
        prop::bool::weighted(0.05),
    )
        .prop_map(
            |(ticks, renders, gpu_ticks, disjoint, frequency, null_handles)| FrameOp {
                ticks,
                renders,
                gpu_ticks,
                disjoint,
                frequency,
                null_handles,
            },
        )
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        2 => any::<bool>().prop_map(Op::Enable),
        2 => any::<bool>().prop_map(Op::GpuEnable),
        // fps_num 0 is excluded: a ring of capacity 0 (the C divides by zero)
        1 => prop_oneof![(1u32..=3, 1u32..=2), (1u32..=240, 1u32..=1001)]
            .prop_map(|(n, d)| Op::Reset(n, d)),
        10 => frame_op().prop_map(Op::Frame),
        2 => (0usize..3, 1u64..u64::MAX).prop_map(|(s, t)| Op::Async(s, t)),
        1 => (0usize..3, 0u64..1_000).prop_map(|(s, t)| Op::LastAsync(s, t)),
        1 => (0usize..3).prop_map(Op::Remove),
    ]
}

/// One sequence on both sides.
struct Pair {
    p: SourceProfiler,
    host: Fake,
    sources: [*mut c_void; 3],
    clock: u64,
}

impl Pair {
    /// The oracle reset and a first video reset (so rings have room).
    fn new(num: u32, den: u32) -> Self {
        // SAFETY: the caller holds ORACLE; the sources are freed in Drop.
        let sources = unsafe {
            oracle_sp_reset();
            SOURCE_FLAGS.map(|f| oracle_sp_source_new(f))
        };
        let host = Fake::new();
        for (i, &s) in sources.iter().enumerate() {
            host.async_flags
                .borrow_mut()
                .insert(s.addr(), SOURCE_FLAGS[i] == OBS_SOURCE_ASYNC_VIDEO);
        }
        let pair = Self {
            p: SourceProfiler::new(),
            host,
            sources,
            clock: 1_000,
        };
        pair.apply(&Op::Reset(num, den));
        pair
    }

    fn push_time(&self, t: u64) {
        self.host.times.borrow_mut().push_back(t);
        // SAFETY: the oracle's clock queue.
        unsafe { oracle_sp_push_time(t) };
    }

    fn timed(&mut self, delta: u64) {
        self.push_time(self.clock);
        self.push_time(self.clock.wrapping_add(delta));
        self.clock = self.clock.wrapping_add(1_000_000_007);
    }

    fn apply(&self, op: &Op) {
        let h = &self.host;
        // SAFETY (whole block): oracle calls on its own sources and stand-ins,
        // under ORACLE.
        unsafe {
            match *op {
                Op::Enable(on) => {
                    oracle_source_profiler_enable(on);
                    self.p.enable(on);
                }
                Op::GpuEnable(on) => {
                    oracle_source_profiler_gpu_enable(on);
                    self.p.gpu_enable(on);
                }
                Op::Reset(num, den) => {
                    let ovi = OracleVideoInfoHead {
                        graphics_module: ptr::null(),
                        fps_num: num,
                        fps_den: den,
                    };
                    oracle_source_profiler_reset_video(&raw const ovi);
                    self.p.reset_video(num, den, h);
                }
                Op::Async(i, ts) => {
                    self.push_time(ts);
                    oracle_source_profiler_async_frame_received(self.sources[i]);
                    self.p.async_frame_received(self.sources[i].addr(), h);
                }
                Op::LastAsync(i, ts) => {
                    oracle_sp_source_set_last_async_ts(self.sources[i], ts);
                    h.last_async.borrow_mut().insert(self.sources[i].addr(), ts);
                }
                Op::Remove(i) => {
                    oracle_source_profiler_remove_source(self.sources[i]);
                    if self.p.should_queue_removal() {
                        self.p.delete_source(self.sources[i].addr(), h);
                    }
                }
                Op::Frame(_) => unreachable!("frames go through frame()"),
            }
        }
    }

    fn frame(&mut self, f: &FrameOp) {
        // SAFETY (whole function): as apply.
        unsafe {
            oracle_sp_set_next_ticks(f.gpu_ticks.is_some(), f.gpu_ticks.unwrap_or(0));
            oracle_sp_set_range(f.disjoint, f.frequency);
            oracle_sp_set_null_handles(f.null_handles);
        }
        self.host.next_ticks.set(f.gpu_ticks);
        self.host.range.set(RangeData {
            disjoint: f.disjoint,
            frequency: f.frequency,
        });
        self.host.null_handles.set(f.null_handles);

        // SAFETY: as apply.
        unsafe { oracle_source_profiler_frame_begin() };
        self.p.frame_begin();
        for i in 0..3 {
            if let Some(delta) = f.ticks[i] {
                self.timed(delta);
                let src = self.sources[i];
                // SAFETY: as apply.
                unsafe {
                    let s = oracle_source_profiler_source_tick_start();
                    oracle_source_profiler_source_tick_end(src, s);
                }
                let s = self.p.source_tick_start(&self.host);
                self.p.source_tick_end(src.addr(), s, &self.host);
            }
        }
        // SAFETY: as apply.
        unsafe { oracle_source_profiler_render_begin() };
        self.p.render_begin(&self.host);
        for i in 0..3 {
            for &delta in &f.renders[i] {
                self.timed(delta);
                let src = self.sources[i];
                // SAFETY: as apply; the timer out-pointer is a local.
                unsafe {
                    let mut timer: *mut c_void = ptr::null_mut();
                    let s = oracle_source_profiler_source_render_begin(&raw mut timer);
                    oracle_source_profiler_source_render_end(src, s, timer);
                }
                let (s, timer) = self.p.source_render_begin(&self.host).unwrap_or((0, 0));
                self.p.source_render_end(src.addr(), s, timer, &self.host);
            }
        }
        // SAFETY: as apply.
        unsafe { oracle_source_profiler_render_end() };
        self.p.render_end(&self.host);
        // SAFETY: as apply.
        unsafe { oracle_source_profiler_frame_collect() };
        self.p.frame_collect(&self.host);
    }

    fn run(&mut self, op: &Op) {
        match op {
            Op::Frame(f) => self.frame(f),
            other => self.apply(other),
        }
    }

    /// Everything observable, compared; `step` names the operation.
    fn compare(&self, step: &str) {
        let h = &self.host;
        // SAFETY (whole function): oracle reads under ORACLE, into locals.
        unsafe {
            let (mut en, mut gpu, mut idx, mut samples) = (false, false, 0u8, 0u64);
            let mut ranges = [0usize; 2];
            oracle_sp_state(
                &raw mut en,
                &raw mut gpu,
                &raw mut idx,
                ranges.as_mut_ptr(),
                &raw mut samples,
            );
            assert_eq!(oracle_sp_frame_buffer_size(), FRAME_BUFFER_SIZE);
            assert_eq!(
                (en, gpu, usize::from(idx), ranges, samples),
                (
                    self.p.is_enabled(),
                    self.p.is_gpu_enabled(),
                    self.p.timer_ranges().0,
                    self.p.timer_ranges().1,
                    self.p.samples_per_entry()
                ),
                "state after {step}"
            );
            assert_eq!(
                oracle_sp_samples_count(),
                self.p.sampled_sources().len(),
                "sample count after {step}"
            );
            assert_eq!(
                oracle_sp_entries_count(),
                self.p.entry_sources().len(),
                "entry count after {step}"
            );

            for (i, &src) in self.sources.iter().enumerate() {
                assert_eq!(
                    oracle_samples(src),
                    self.p.samples(src.addr()).map(|s| {
                        (
                            s.frame_idx,
                            s.frames
                                .map(|f| (f.tick, f.render_cpu, f.render_timers))
                                .to_vec(),
                        )
                    }),
                    "samples of source {i} after {step}"
                );
                assert_eq!(
                    oracle_entry(src),
                    self.p.entry(src.addr()).map(|e| rings(&e)),
                    "entry of source {i} after {step}"
                );
                assert_eq!(
                    oracle_fill(src),
                    rust_fill(self.p.fill_result(src.addr(), h)),
                    "fill_result of source {i} after {step}"
                );
            }
            let unknown = 0x77usize;
            assert_eq!(
                oracle_fill(ptr::with_exposed_provenance_mut(unknown)),
                rust_fill(self.p.fill_result(unknown, h)),
                "fill_result of an unknown source after {step}"
            );

            let mut out = vec![0usize; 4096];
            let n = oracle_sp_take_destroyed_timers(out.as_mut_ptr(), out.len());
            let mut c_timers = out[..n].to_vec();
            let mut r_timers = std::mem::take(&mut *h.destroyed_timers.borrow_mut());
            c_timers.sort_unstable();
            r_timers.sort_unstable();
            assert_eq!(c_timers, r_timers, "destroyed timers after {step}");
            let n = oracle_sp_take_destroyed_ranges(out.as_mut_ptr(), out.len());
            let mut c_ranges = out[..n].to_vec();
            let mut r_ranges = std::mem::take(&mut *h.destroyed_ranges.borrow_mut());
            c_ranges.sort_unstable();
            r_ranges.sort_unstable();
            assert_eq!(c_ranges, r_ranges, "destroyed ranges after {step}");

            assert_eq!(
                oracle_sp_warnings(),
                h.warnings.get(),
                "warnings after {step}"
            );
            assert_eq!(oracle_sp_context_depth(), 0, "C context after {step}");
            assert_eq!(h.depth.get(), 0, "Rust context after {step}");
            assert_eq!(
                oracle_sp_pending_times(),
                h.times.borrow().len(),
                "clock readings after {step}"
            );
            while oracle_sp_pending_times() > 0 {
                oracle_sp_gettime_ns();
            }
            h.times.borrow_mut().clear();
        }
    }
}

impl Drop for Pair {
    fn drop(&mut self) {
        // SAFETY: under ORACLE; drop the oracle's state before its sources.
        unsafe {
            oracle_sp_reset();
            for s in self.sources {
                oracle_sp_source_free(s);
            }
        }
    }
}

type SampleView = (usize, Vec<(u64, Vec<u64>, Vec<Handle>)>);
type RingView = (Vec<u64>, usize, usize);

/// # Safety
///
/// Under ORACLE.
unsafe fn oracle_samples(src: *mut c_void) -> Option<SampleView> {
    let mut frames = Vec::new();
    let mut frame_idx = 0u8;
    for frame in 0..FRAME_BUFFER_SIZE {
        let (mut tick, mut cpu_num, mut timers_num) = (0u64, 0usize, 0usize);
        let mut cpu = vec![0u64; 64];
        let mut timers = vec![0usize; 64];
        // SAFETY: locals sized as passed.
        let found = unsafe {
            oracle_sp_sample(
                src,
                frame,
                &raw mut frame_idx,
                &raw mut tick,
                cpu.as_mut_ptr(),
                cpu.len(),
                &raw mut cpu_num,
                timers.as_mut_ptr(),
                timers.len(),
                &raw mut timers_num,
            )
        };
        if !found {
            return None;
        }
        cpu.truncate(cpu_num);
        timers.truncate(timers_num);
        frames.push((tick, cpu, timers));
    }
    Some((usize::from(frame_idx), frames))
}

/// # Safety
///
/// Under ORACLE.
unsafe fn oracle_entry(src: *mut c_void) -> Option<Vec<RingView>> {
    let mut out = Vec::new();
    for which in 0..RINGS {
        let mut values = vec![0u64; 4096];
        let (mut num, mut idx, mut cap) = (0usize, 0usize, 0usize);
        // SAFETY: locals sized as passed.
        let found = unsafe {
            oracle_sp_entry_ring(
                src,
                which as core::ffi::c_int,
                values.as_mut_ptr(),
                values.len(),
                &raw mut num,
                &raw mut idx,
                &raw mut cap,
            )
        };
        if !found {
            return None;
        }
        values.truncate(num);
        out.push((values, idx, cap));
    }
    Some(out)
}

fn rings(e: &Entry) -> Vec<RingView> {
    [
        &e.tick,
        &e.render_cpu,
        &e.render_gpu,
        &e.render_cpu_sum,
        &e.render_gpu_sum,
        &e.async_frame_ts,
        &e.async_rendered_ts,
    ]
    .iter()
    .map(|b| (b.values().to_vec(), b.idx(), b.capacity()))
    .collect()
}

/// What a C `fill_result` call shows: the return value and the result's
/// bytes (a sentinel shows an untouched result).
type FillView = (bool, [u64; 14]);

const SENTINEL: u64 = 0xABAB_ABAB_ABAB_ABAB;

fn result_words(r: &OracleProfilerResult) -> [u64; 14] {
    [
        r.tick_avg,
        r.tick_max,
        r.render_avg,
        r.render_max,
        r.render_gpu_avg,
        r.render_gpu_max,
        r.render_sum,
        r.render_gpu_sum,
        r.async_input.to_bits(),
        r.async_rendered.to_bits(),
        r.async_input_best,
        r.async_input_worst,
        r.async_rendered_best,
        r.async_rendered_worst,
    ]
}

fn sentinel() -> OracleProfilerResult {
    let s = SENTINEL;
    let f = f64::from_bits(SENTINEL);
    OracleProfilerResult {
        tick_avg: s,
        tick_max: s,
        render_avg: s,
        render_max: s,
        render_gpu_avg: s,
        render_gpu_max: s,
        render_sum: s,
        render_gpu_sum: s,
        async_input: f,
        async_rendered: f,
        async_input_best: s,
        async_input_worst: s,
        async_rendered_best: s,
        async_rendered_worst: s,
    }
}

/// # Safety
///
/// Under ORACLE.
unsafe fn oracle_fill(src: *mut c_void) -> FillView {
    let mut r = sentinel();
    // SAFETY: a local result.
    let ok = unsafe { oracle_source_profiler_fill_result(src, &raw mut r) };
    (ok, result_words(&r))
}

fn to_oracle(r: &ProfilerResult) -> OracleProfilerResult {
    OracleProfilerResult {
        tick_avg: r.tick_avg,
        tick_max: r.tick_max,
        render_avg: r.render_avg,
        render_max: r.render_max,
        render_gpu_avg: r.render_gpu_avg,
        render_gpu_max: r.render_gpu_max,
        render_sum: r.render_sum,
        render_gpu_sum: r.render_gpu_sum,
        async_input: r.async_input,
        async_rendered: r.async_rendered,
        async_input_best: r.async_input_best,
        async_input_worst: r.async_input_worst,
        async_rendered_best: r.async_rendered_best,
        async_rendered_worst: r.async_rendered_worst,
    }
}

fn rust_fill(fill: Fill) -> FillView {
    match fill {
        Fill::Disabled => (false, result_words(&sentinel())),
        Fill::Missing => (false, [0; 14]),
        Fill::Found(r) => (true, result_words(&to_oracle(&r))),
    }
}

fn run_sequence(reset: (u32, u32), ops: &[Op]) {
    let _guard = ORACLE.lock().unwrap_or_else(|e| e.into_inner());
    let mut pair = Pair::new(reset.0, reset.1);
    pair.compare("the first video reset");
    for (n, op) in ops.iter().enumerate() {
        pair.run(op);
        pair.compare(&format!("op {n}: {op:?}"));
    }
}

// ---------------------------------------------------------------------------
// fixed sequences

fn frame_all(tick: u64, render: u64, gpu_ticks: Option<u64>) -> Op {
    Op::Frame(FrameOp {
        ticks: [Some(tick); 3],
        renders: [vec![render, render / 2], vec![render], vec![]],
        gpu_ticks,
        disjoint: false,
        frequency: 1_000_000,
        null_handles: false,
    })
}

#[test]
fn parity_cpu_frames_fill_and_wrap_rings() {
    let mut ops = vec![Op::Enable(true)];
    for n in 0..12 {
        ops.push(frame_all(10 + n, 100 + 7 * n, None));
        ops.push(Op::Async(1, 1_000 + 33 * n * n));
        ops.push(Op::LastAsync(1, 500 + 10 * n));
    }
    ops.push(Op::Remove(2));
    ops.push(frame_all(1, 1, None));
    ops.push(Op::Enable(false));
    ops.push(frame_all(1, 1, None));
    // 1/2 fps: rings of 5, wrapped twice
    run_sequence((1, 2), &ops);
}

#[test]
fn parity_gpu_frames() {
    let mut ops = vec![Op::Enable(true), Op::GpuEnable(true)];
    for n in 0..8 {
        ops.push(frame_all(10, 100, Some(1_000 * (n + 1))));
    }
    ops.push(Op::Frame(FrameOp {
        ticks: [Some(1); 3],
        renders: [vec![1], vec![2], vec![3]],
        gpu_ticks: None,
        disjoint: true,
        frequency: 1,
        null_handles: false,
    }));
    ops.push(frame_all(10, 100, Some(7)));
    ops.push(Op::GpuEnable(false));
    ops.push(frame_all(10, 100, Some(7)));
    ops.push(frame_all(10, 100, Some(7)));
    ops.push(Op::Remove(0));
    ops.push(Op::Reset(30, 1));
    run_sequence((30, 1), &ops);
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    #[test]
    fn parity_random_sequences(
        reset in prop_oneof![(1u32..=3, 1u32..=2), (1u32..=120, 1u32..=1001)],
        ops in prop::collection::vec(op(), 0..60),
    ) {
        run_sequence(reset, &ops);
    }

    #[test]
    fn parity_ucirclebuf(
        // capacity 0 is excluded: the C divides by zero on push
        capacity in 1usize..9,
        values in prop::collection::vec(any::<u64>(), 0..40),
    ) {
        let mut out = vec![0u64; capacity];
        let (mut num, mut idx) = (0usize, 0usize);
        // SAFETY: values and out sized as passed; no shared state.
        unsafe {
            oracle_sp_ucirclebuf_run(
                capacity,
                values.as_ptr(),
                values.len(),
                out.as_mut_ptr(),
                &raw mut num,
                &raw mut idx,
            );
        }
        out.truncate(num);
        let mut b = CircleBuf::new(capacity);
        for &v in &values {
            b.push(v);
        }
        prop_assert_eq!((b.values().to_vec(), b.idx(), b.capacity()), (out, idx, capacity));
    }

    #[test]
    fn parity_calculate(
        tick in prop::collection::vec(any::<u64>(), 0..12),
        cpu in prop::collection::vec((any::<u64>(), any::<u64>()), 0..12),
        gpu in prop::collection::vec((0u64..1 << 40, 0u64..1 << 40), 0..12),
        gpu_on in any::<bool>(),
        frames in prop::collection::vec(
            // small values repeat, so equal neighbours are common
            prop_oneof![3 => 1u64..1_000_000, 2 => 1u64..4, 1 => Just(0u64), 1 => any::<u64>()],
            0..12),
    ) {
        let mut tick_c = tick.clone();
        let (mut cpu_c, mut cpu_sum_c): (Vec<u64>, Vec<u64>) = cpu.iter().copied().unzip();
        let (mut gpu_c, mut gpu_sum_c): (Vec<u64>, Vec<u64>) = gpu.iter().copied().unzip();
        let mut frames_c = frames.clone();
        let mut c = OracleProfilerResult::default();
        // oracle_sp_calculate sets the oracle's static gpu_enabled for the
        // call, so it holds ORACLE like the sequences do (#1930).
        let _guard = ORACLE.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: each array sized as passed, read only; the oracle's
        // statics are under ORACLE.
        unsafe {
            oracle_sp_calculate(
                tick_c.as_mut_ptr(),
                tick_c.len(),
                cpu_c.as_mut_ptr(),
                cpu_sum_c.as_mut_ptr(),
                cpu_c.len(),
                gpu_c.as_mut_ptr(),
                gpu_sum_c.as_mut_ptr(),
                gpu_c.len(),
                gpu_on,
                frames_c.as_mut_ptr(),
                frames_c.len(),
                &raw mut c,
            );
        }
        let mut r = ProfilerResult::default();
        calculate_tick(&tick_c, &mut r);
        calculate_render(
            &cpu_c,
            &cpu_sum_c,
            gpu_on.then_some((gpu_c.as_slice(), gpu_sum_c.as_slice())),
            &mut r,
        );
        if let Some(fps) = calculate_fps(&frames_c) {
            r.async_input = fps.fps;
            r.async_input_best = fps.best;
            r.async_input_worst = fps.worst;
        }
        prop_assert_eq!(result_words(&to_oracle(&r)), result_words(&c));
    }
}

// ---------------------------------------------------------------------------
// Tier 2 layout

#[test]
fn profiler_result_layout_matches_c() {
    // SAFETY: pure size/offset queries.
    unsafe {
        assert_eq!(size_of::<profiler_result>(), oracle_sp_result_size());
        assert_eq!(align_of::<profiler_result>(), oracle_sp_result_align());
        let offsets = [
            core::mem::offset_of!(profiler_result, tick_avg),
            core::mem::offset_of!(profiler_result, tick_max),
            core::mem::offset_of!(profiler_result, render_avg),
            core::mem::offset_of!(profiler_result, render_max),
            core::mem::offset_of!(profiler_result, render_gpu_avg),
            core::mem::offset_of!(profiler_result, render_gpu_max),
            core::mem::offset_of!(profiler_result, render_sum),
            core::mem::offset_of!(profiler_result, render_gpu_sum),
            core::mem::offset_of!(profiler_result, async_input),
            core::mem::offset_of!(profiler_result, async_rendered),
            core::mem::offset_of!(profiler_result, async_input_best),
            core::mem::offset_of!(profiler_result, async_input_worst),
            core::mem::offset_of!(profiler_result, async_rendered_best),
            core::mem::offset_of!(profiler_result, async_rendered_worst),
        ];
        for (field, offset) in offsets.into_iter().enumerate() {
            assert_eq!(
                offset,
                oracle_sp_result_offset(field as core::ffi::c_int),
                "field {field}"
            );
        }
        assert_eq!(
            core::mem::offset_of!(obs_video_info_head, fps_num),
            oracle_sp_ovi_fps_num_offset()
        );
        assert_eq!(
            core::mem::offset_of!(obs_video_info_head, fps_den),
            oracle_sp_ovi_fps_den_offset()
        );
    }
}

// ---------------------------------------------------------------------------
// the shim, with the cargo-test stand-ins (no graphics, NULL timers)

/// The only test that touches the shim's process-wide profiler.
#[test]
fn shim_runs_without_graphics() {
    let mut a = 0u8;
    let mut b = 0u8;
    let src: *mut c_void = (&raw mut a).cast();
    let other: *mut c_void = (&raw mut b).cast();
    let mut r = profiler_result {
        tick_avg: 7,
        ..profiler_result::default()
    };
    let ovi = obs_video_info_head {
        graphics_module: ptr::null(),
        fps_num: 30,
        fps_den: 1,
    };

    // SAFETY (whole block): the shims with valid locals; the sources are
    // only keys, never dereferenced by the stand-ins.
    unsafe {
        source_profiler_reset_video(&raw const ovi);
        assert!(!source_profiler_fill_result(src, &raw mut r));
        assert_eq!(r.tick_avg, 7, "disabled: untouched");
        assert!(source_profiler_get_result(src).is_null());
        assert_eq!(source_profiler_source_tick_start(), 0);

        source_profiler_gpu_enable(true);
        source_profiler_enable(true);
        source_profiler_gpu_enable(true);
        for _ in 0..3 {
            source_profiler_frame_begin();
            let start = source_profiler_source_tick_start();
            source_profiler_source_tick_end(src, start);
            source_profiler_render_begin();
            let mut timer: *mut c_void = other;
            let start = source_profiler_source_render_begin(&raw mut timer);
            assert!(timer.is_null(), "gs_timer_create stand-in returns NULL");
            source_profiler_source_render_end(src, start, timer);
            // a NULL out-pointer is not written (intentional difference)
            let start = source_profiler_source_render_begin(ptr::null_mut());
            source_profiler_source_render_end(src, start, ptr::null_mut());
            source_profiler_render_end();
            source_profiler_frame_collect();
        }
        source_profiler_async_frame_received(src);

        assert!(source_profiler_fill_result(src, &raw mut r));
        assert!(r.render_sum >= r.render_avg);
        assert_eq!(r.async_input, 0.0, "the stand-in has no async flag");
        assert!(!source_profiler_fill_result(src, ptr::null_mut()));
        let got = source_profiler_get_result(src);
        assert!(!got.is_null());
        bfree(got.cast());

        r.tick_avg = 7;
        assert!(!source_profiler_fill_result(other, &raw mut r));
        assert_eq!(r, profiler_result::default(), "missing: zeroed");

        // the stand-in task queue runs the removal at once
        source_profiler_remove_source(src);
        assert!(!source_profiler_fill_result(src, &raw mut r));

        source_profiler_enable(false);
        source_profiler_frame_begin();
        source_profiler_frame_collect();
        r.tick_avg = 7;
        assert!(!source_profiler_fill_result(src, &raw mut r));
        assert_eq!(r.tick_avg, 7);
        source_profiler_remove_source(src);
    }
}

// ---------------------------------------------------------------------------
// the shim's timer and timer-range calls, with obs-graphics linked

unsafe extern "C" {
    // The entry points the shim calls (ffi/source_profiler.rs), by the same
    // names: obs-graphics defines them, so a cargo-test stand-in for them
    // would be a second definition.
    fn gs_timer_destroy(timer: *mut c_void);
    fn gs_timer_range_destroy(range: *mut c_void);
    // The obs-c-oracle timer fixtures that obs-graphics' entry points log
    // through (oracle/timer_dispatch.c, oracle/timer_range_dispatch.c).
    fn oracle_td_run(
        context: bool,
        timer: bool,
        ticks: bool,
        op: u32,
        result: bool,
        write: bool,
        value: u64,
        output: *mut u64,
    ) -> bool;
    fn oracle_td_log() -> *const std::ffi::c_char;
    fn oracle_trd_run(
        context: bool,
        range: bool,
        disjoint: bool,
        frequency: bool,
        op: u32,
        result: bool,
        writes: u32,
        value_disjoint: bool,
        value: u64,
        out_disjoint: *mut bool,
        out_frequency: *mut u64,
    ) -> bool;
    fn oracle_trd_log() -> *const std::ffi::c_char;
}

/// `gs_timer_*` and `gs_timer_range_*` resolve to obs-graphics' dispatch,
/// not to do-nothing stand-ins: with no graphics context they log the
/// same debug line libobs does. Two definitions of these names broke the
/// `port_comparison` link (#4786, #4615).
#[test]
fn shim_timers_are_obs_graphics_dispatch() {
    let mut ticks = 0u64;
    let mut disjoint = false;
    let mut frequency = 0u64;
    // SAFETY: op 4 runs no entry point; it only clears this thread's
    // fixture logs and leaves no graphics context. The entry points take
    // NULL handles and return before touching them.
    let (timer_log, range_log) = unsafe {
        oracle_td_run(false, false, false, 4, false, false, 0, &raw mut ticks);
        oracle_trd_run(
            false,
            false,
            false,
            false,
            4,
            false,
            0,
            false,
            0,
            &raw mut disjoint,
            &raw mut frequency,
        );
        gs_timer_destroy(ptr::null_mut());
        gs_timer_range_destroy(ptr::null_mut());
        (
            std::ffi::CStr::from_ptr(oracle_td_log()).to_owned(),
            std::ffi::CStr::from_ptr(oracle_trd_log()).to_owned(),
        )
    };
    assert_eq!(
        timer_log.to_str(),
        Ok("gs_timer_destroy: called while not in a graphics context")
    );
    assert_eq!(
        range_log.to_str(),
        Ok("gs_timer_range_destroy: called while not in a graphics context")
    );
}
