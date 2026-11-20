//! Safe mirrors of test_video_stats_queries.c (#5171, #5364), over stats
//! guarded by a real mutex as the C's `struct obs_video_stats` is. Each
//! query must take the lock once, read only its own field, return it
//! unchanged and leave the lock free; values are compared exactly (the
//! FPS by its bits).
use std::sync::Mutex;

use obs_core::video_stats_queries::{
    VideoStats, active_fps, average_frame_time_ns, lagged_frames, total_frames, video_frame_time,
};
use proptest::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Values {
    frame_time_ns: u64,
    avg_frame_time_ns: u64,
    fps: f64,
    total_frames: u32,
    lagged_frames: u32,
}

/// `struct obs_video_stats`: a mutex over the five fields, plus a log of
/// which field each locked read took.
#[derive(Default)]
struct Stats {
    values: Mutex<Values>,
    reads: String,
}

impl Stats {
    fn load(&self, v: Values) {
        *self.values.lock().unwrap() = v;
    }
    /// obs_video_stats_set_frame_time.
    fn set_frame_time(&self, frame_time: u64) {
        self.values.lock().unwrap().frame_time_ns = frame_time;
    }
    /// obs_video_stats_set_timing.
    fn set_timing(&self, fps: f64, frame_time_ns: u64) {
        let mut v = self.values.lock().unwrap();
        v.fps = fps;
        v.avg_frame_time_ns = frame_time_ns;
    }
    /// obs_video_stats_add_frames.
    fn add_frames(&self, total: u32, lagged: u32) {
        let mut v = self.values.lock().unwrap();
        v.total_frames = v.total_frames.wrapping_add(total);
        v.lagged_frames = v.lagged_frames.wrapping_add(lagged);
    }
    fn read<T>(&mut self, field: char, f: impl FnOnce(&Values) -> T) -> T {
        self.reads.push(field);
        let v = self.values.lock().unwrap();
        f(&v)
    }
    fn assert_unlocked(&self) {
        assert!(self.values.try_lock().is_ok(), "stats mutex still held");
    }
}

impl VideoStats for Stats {
    fn frame_time(&mut self) -> u64 {
        self.read('t', |v| v.frame_time_ns)
    }
    fn fps(&mut self) -> f64 {
        self.read('f', |v| v.fps)
    }
    fn avg_frame_time_ns(&mut self) -> u64 {
        self.read('a', |v| v.avg_frame_time_ns)
    }
    fn total_frames(&mut self) -> u32 {
        self.read('n', |v| v.total_frames)
    }
    fn lagged_frames(&mut self) -> u32 {
        self.read('l', |v| v.lagged_frames)
    }
}

/// assert_queries: each query reads its own field once and unlocks.
fn assert_queries(stats: &mut Stats, v: &Values) {
    stats.reads.clear();
    assert_eq!(video_frame_time(stats), v.frame_time_ns);
    stats.assert_unlocked();
    assert_eq!(active_fps(stats).to_bits(), v.fps.to_bits());
    stats.assert_unlocked();
    assert_eq!(average_frame_time_ns(stats), v.avg_frame_time_ns);
    stats.assert_unlocked();
    assert_eq!(total_frames(stats), v.total_frames);
    stats.assert_unlocked();
    assert_eq!(lagged_frames(stats), v.lagged_frames);
    stats.assert_unlocked();
    assert_eq!(stats.reads, "tfanl");
}

/// test_zeroed_stats.
#[test]
fn zeroed_stats() {
    assert_queries(&mut Stats::default(), &Values::default());
}

/// test_each_query_reads_its_field.
#[test]
fn each_query_reads_its_field() {
    let v = Values {
        frame_time_ns: 0x1122_3344_5566_7788,
        avg_frame_time_ns: 16_666_667,
        fps: 59.94,
        total_frames: 123_456,
        lagged_frames: 7,
    };
    let mut stats = Stats::default();
    stats.load(v);
    assert_queries(&mut stats, &v);
}

/// test_boundary_values.
#[test]
fn boundary_values() {
    let v = Values {
        frame_time_ns: u64::MAX,
        avg_frame_time_ns: u64::MAX - 1,
        fps: 1e308,
        total_frames: u32::MAX,
        lagged_frames: u32::MAX - 1,
    };
    let mut stats = Stats::default();
    stats.load(v);
    assert_queries(&mut stats, &v);
}

/// test_reads_current_core: the queries read whichever stats they are given.
#[test]
fn reads_current_core() {
    let a = Values {
        frame_time_ns: 1,
        avg_frame_time_ns: 2,
        fps: 30.0,
        total_frames: 4,
        lagged_frames: 5,
    };
    let b = Values {
        frame_time_ns: 10,
        avg_frame_time_ns: 20,
        fps: 0.5,
        total_frames: 40,
        lagged_frames: 50,
    };
    let mut core_a = Stats::default();
    let mut core_b = Stats::default();
    core_a.load(a);
    core_b.load(b);
    assert_queries(&mut core_a, &a);
    assert_queries(&mut core_b, &b);
    assert_queries(&mut core_a, &a);
}

/// test_reads_setter_writes.
#[test]
fn reads_setter_writes() {
    let mut stats = Stats::default();
    stats.set_frame_time(1000);
    stats.set_timing(29.97, 33_366_700);
    stats.add_frames(10, 3);
    stats.add_frames(5, 1);
    let v = Values {
        frame_time_ns: 1000,
        avg_frame_time_ns: 33_366_700,
        fps: 29.97,
        total_frames: 15,
        lagged_frames: 4,
    };
    assert_queries(&mut stats, &v);
}

proptest! {
    /// Any stored values, the FPS as any bit pattern, pass through unchanged.
    #[test]
    fn any_values_pass_through(
        frame_time_ns in any::<u64>(),
        avg_frame_time_ns in any::<u64>(),
        fps_bits in any::<u64>(),
        total_frames in any::<u32>(),
        lagged_frames in any::<u32>(),
    ) {
        let v = Values { frame_time_ns, avg_frame_time_ns, fps: f64::from_bits(fps_bits), total_frames, lagged_frames };
        let mut stats = Stats::default();
        stats.load(v);
        assert_queries(&mut stats, &v);
    }
}
