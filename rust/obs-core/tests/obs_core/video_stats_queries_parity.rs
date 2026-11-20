//! Original-C parity for the video-statistics queries (#5364): the thin ABI
//! shims and real adapters against the extracted C, on one real-header core
//! with an initialized stats mutex. Compared: each query's exact result bits
//! (the FPS as any binary64 pattern), the mutex free after every query, the
//! stats values unchanged and the current core pointer untouched. Excludes a
//! NULL or dead core and concurrent stats writes, matching the C contract.
//! No intentional differences.
use obs_core::ffi::video_stats_queries::{
    obs_get_active_fps, obs_get_average_frame_time_ns, obs_get_lagged_frames, obs_get_total_frames,
    obs_get_video_frame_time,
};
use proptest::prelude::*;

#[repr(C)]
#[derive(Clone, Copy)]
struct Candidates {
    frame_time: unsafe extern "C" fn() -> u64,
    fps: unsafe extern "C" fn() -> f64,
    avg_frame_time_ns: unsafe extern "C" fn() -> u64,
    total_frames: unsafe extern "C" fn() -> u32,
    lagged_frames: unsafe extern "C" fn() -> u32,
}

const SHIMS: Candidates = Candidates {
    frame_time: obs_get_video_frame_time,
    fps: obs_get_active_fps,
    avg_frame_time_ns: obs_get_average_frame_time_ns,
    total_frames: obs_get_total_frames,
    lagged_frames: obs_get_lagged_frames,
};

unsafe extern "C" {
    fn oracle_video_stats_queries_adapter() -> bool;
    fn oracle_video_stats_queries_probe(
        cand: *const Candidates,
        frame_time_ns: u64,
        avg_frame_time_ns: u64,
        fps_bits: u64,
        total_frames: u32,
        lagged_frames: u32,
        out: *mut u64,
    ) -> u32;
}

#[derive(Clone, Copy, Debug)]
struct Values {
    frame_time_ns: u64,
    avg_frame_time_ns: u64,
    fps_bits: u64,
    total_frames: u32,
    lagged_frames: u32,
}

fn probe(cand: &Candidates, v: Values) -> (u32, [u64; 5]) {
    let mut out = [0u64; 5];
    // SAFETY: the oracle owns a live real-header core with initialized
    // stats, installs it as the thread's current core for the calls and
    // writes exactly five results.
    let diff = unsafe {
        oracle_video_stats_queries_probe(
            cand,
            v.frame_time_ns,
            v.avg_frame_time_ns,
            v.fps_bits,
            v.total_frames,
            v.lagged_frames,
            out.as_mut_ptr(),
        )
    };
    (diff, out)
}

fn check(v: Values) {
    let (diff, out) = probe(&SHIMS, v);
    assert_eq!(diff, 0, "{v:?}");
    // The original returns every stored value unchanged.
    assert_eq!(
        out,
        [
            v.frame_time_ns,
            v.fps_bits,
            v.avg_frame_time_ns,
            u64::from(v.total_frames),
            u64::from(v.lagged_frames),
        ]
    );
}

#[test]
fn real_adapter_addresses_the_core_stats() {
    // SAFETY: the oracle allocates, installs and frees its own core.
    assert!(unsafe { oracle_video_stats_queries_adapter() });
}

#[test]
fn characterized_values_match_original_c() {
    let cases = [
        Values {
            frame_time_ns: 0,
            avg_frame_time_ns: 0,
            fps_bits: 0,
            total_frames: 0,
            lagged_frames: 0,
        },
        Values {
            frame_time_ns: 0x1122_3344_5566_7788,
            avg_frame_time_ns: 16_666_667,
            fps_bits: 59.94f64.to_bits(),
            total_frames: 123_456,
            lagged_frames: 7,
        },
        Values {
            frame_time_ns: u64::MAX,
            avg_frame_time_ns: u64::MAX - 1,
            fps_bits: 1e308f64.to_bits(),
            total_frames: u32::MAX,
            lagged_frames: u32::MAX - 1,
        },
        Values {
            frame_time_ns: 1000,
            avg_frame_time_ns: 33_366_700,
            fps_bits: 29.97f64.to_bits(),
            total_frames: 15,
            lagged_frames: 4,
        },
    ];
    for v in cases {
        check(v);
    }
    // Signed zero, infinities and a NaN payload pass through by their bits.
    for fps_bits in [
        0x8000_0000_0000_0000,
        0x7ff0_0000_0000_0000,
        0xfff0_0000_0000_0000,
        0x7ff8_0000_0001_2345,
        0x0000_0000_0000_0001,
    ] {
        check(Values {
            fps_bits,
            ..cases[1]
        });
    }
}

// Shim mutants, each of which the probe must catch.
unsafe extern "C" fn mutant_fps_from_average() -> f64 {
    // SAFETY: forwarded; the probe's core is live.
    let ns = unsafe { obs_get_average_frame_time_ns() };
    if ns == 0 { 0.0 } else { 1e9 / ns as f64 }
}
unsafe extern "C" fn mutant_total_reads_lagged() -> u32 {
    // SAFETY: forwarded; the probe's core is live.
    unsafe { obs_get_lagged_frames() }
}
unsafe extern "C" fn mutant_lagged_narrowed() -> u32 {
    // SAFETY: forwarded; the probe's core is live.
    let lagged = unsafe { obs_get_lagged_frames() };
    lagged & 0xffff
}
unsafe extern "C" fn mutant_frame_time_is_average() -> u64 {
    // SAFETY: forwarded; the probe's core is live.
    unsafe { obs_get_average_frame_time_ns() }
}

#[test]
fn mutants_are_caught() {
    let v = Values {
        frame_time_ns: 0x1122_3344_5566_7788,
        avg_frame_time_ns: 16_666_667,
        fps_bits: 59.94f64.to_bits(),
        total_frames: 123_456,
        lagged_frames: 0x0001_0007,
    };
    let mutants = [
        Candidates {
            fps: mutant_fps_from_average,
            ..SHIMS
        },
        Candidates {
            total_frames: mutant_total_reads_lagged,
            ..SHIMS
        },
        Candidates {
            lagged_frames: mutant_lagged_narrowed,
            ..SHIMS
        },
        Candidates {
            frame_time: mutant_frame_time_is_average,
            ..SHIMS
        },
    ];
    for (i, mutant) in mutants.iter().enumerate() {
        let (diff, _) = probe(mutant, v);
        assert_ne!(diff, 0, "mutant {i} not caught");
    }
}

// Signaling NaNs are excluded: binary64 returns through x87 on 32-bit x86
// may quiet them in both implementations alike.
fn not_signaling_nan(bits: u64) -> bool {
    let nan = (bits & 0x7ff0_0000_0000_0000) == 0x7ff0_0000_0000_0000
        && (bits & 0x000f_ffff_ffff_ffff) != 0;
    !nan || bits & 0x0008_0000_0000_0000 != 0
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn random_values_match_original_c(
        frame_time_ns in any::<u64>(),
        avg_frame_time_ns in any::<u64>(),
        fps_bits in any::<u64>().prop_filter("signaling NaN", |b| not_signaling_nan(*b)),
        total_frames in any::<u32>(),
        lagged_frames in any::<u32>(),
    ) {
        check(Values { frame_time_ns, avg_frame_time_ns, fps_bits, total_frames, lagged_frames });
    }
}
