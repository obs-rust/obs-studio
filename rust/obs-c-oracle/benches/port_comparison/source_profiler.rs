//! `libobs/util/source-profiler.c.h`: one frame of the source profiler,
//! as the graphics thread runs it, for 16 sources (a tick and two render
//! passes each, CPU timing only), then `fill_result` for each. The C oracle
//! runs with its stand-ins (a clock advancing 1 µs per reading); the Rust
//! core with a host doing the same. Both profilers run at 30 fps, so their
//! rings hold 150 frames and wrap during the measurement.

use std::cell::Cell;
use std::ffi::c_void;
use std::hint::black_box;
use std::ptr;

use criterion::{Criterion, Throughput};
use obs_c_oracle::source_profiler as c_sp;
use obs_core::source_profiler::{Fill, Handle, Host, RangeData, SourceProfiler};

const SOURCES: usize = 16;
const STEP: u64 = 1_000;

struct BenchHost {
    clock: Cell<u64>,
}

impl Host for BenchHost {
    fn now_ns(&self) -> u64 {
        self.clock.set(self.clock.get() + STEP);
        self.clock.get()
    }
    fn enter_graphics(&self) {}
    fn leave_graphics(&self) {}
    fn timer_create(&self) -> Handle {
        0
    }
    fn timer_begin(&self, _: Handle) {}
    fn timer_end(&self, _: Handle) {}
    fn timer_get_data(&self, _: Handle) -> Option<u64> {
        None
    }
    fn timer_destroy(&self, _: Handle) {}
    fn range_create(&self) -> Handle {
        0
    }
    fn range_begin(&self, _: Handle) {}
    fn range_end(&self, _: Handle) {}
    fn range_get_data(&self, _: Handle) -> RangeData {
        RangeData::default()
    }
    fn range_destroy(&self, _: Handle) {}
    fn is_async_video(&self, _: Handle) -> bool {
        false
    }
    fn last_async_ts(&self, _: Handle) -> u64 {
        0
    }
    fn warn_disjoint(&self) {}
    fn collect_scope_begin(&self) {}
    fn collect_scope_end(&self) {}
}

pub fn benchmarks(criterion: &mut Criterion) {
    // SAFETY: the bench is single-threaded; the oracle's state and sources
    // are this function's until it returns, which frees the sources.
    let sources: Vec<*mut c_void> = unsafe {
        c_sp::oracle_sp_reset();
        c_sp::oracle_sp_set_clock_step(STEP);
        (0..SOURCES)
            .map(|_| c_sp::oracle_sp_source_new(1))
            .collect()
    };
    let ovi = c_sp::OracleVideoInfoHead {
        graphics_module: ptr::null(),
        fps_num: 30,
        fps_den: 1,
    };
    // SAFETY: as above.
    unsafe {
        c_sp::oracle_source_profiler_reset_video(&raw const ovi);
        c_sp::oracle_source_profiler_enable(true);
    }
    let host = BenchHost {
        clock: Cell::new(0),
    };
    let rust = SourceProfiler::new();
    rust.reset_video(30, 1, &host);
    rust.enable(true);

    let mut group = criterion.benchmark_group("source_profiler_frame");
    group.throughput(Throughput::Elements(SOURCES as u64));
    group.bench_function("rust", |b| {
        b.iter(|| {
            rust.frame_begin();
            for &s in &sources {
                let start = rust.source_tick_start(&host);
                rust.source_tick_end(s.addr(), start, &host);
            }
            for &s in &sources {
                for _ in 0..2 {
                    let (start, timer) = rust.source_render_begin(&host).unwrap_or((0, 0));
                    rust.source_render_end(s.addr(), start, timer, &host);
                }
            }
            rust.frame_collect(&host);
            let mut found = 0;
            for &s in &sources {
                if let Fill::Found(r) = rust.fill_result(s.addr(), &host) {
                    found += black_box(r).tick_avg;
                }
            }
            found
        })
    });
    group.bench_function("c", |b| {
        // SAFETY: as above; the timer and result out-pointers are locals.
        b.iter(|| unsafe {
            c_sp::oracle_source_profiler_frame_begin();
            for &s in &sources {
                let start = c_sp::oracle_source_profiler_source_tick_start();
                c_sp::oracle_source_profiler_source_tick_end(s, start);
            }
            for &s in &sources {
                for _ in 0..2 {
                    let mut timer: *mut c_void = ptr::null_mut();
                    let start = c_sp::oracle_source_profiler_source_render_begin(&raw mut timer);
                    c_sp::oracle_source_profiler_source_render_end(s, start, timer);
                }
            }
            c_sp::oracle_source_profiler_frame_collect();
            let mut found = 0;
            for &s in &sources {
                let mut r = c_sp::OracleProfilerResult::default();
                if c_sp::oracle_source_profiler_fill_result(s, &raw mut r) {
                    found += black_box(r).tick_avg;
                }
            }
            found
        })
    });
    group.finish();
    // SAFETY: as above; the reset drops the profiler's entries for the
    // sources first, then each source is freed once.
    unsafe {
        c_sp::oracle_sp_reset();
        for source in sources {
            c_sp::oracle_sp_source_free(source);
        }
    }
}
