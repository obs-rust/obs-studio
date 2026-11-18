//! Tier 3: original C `obs_output_video()` (with its original C dependencies)
//! against the Rust export over real-header fixtures: borrowed identity,
//! diagnostics and unchanged output/encoder state. No intentional differences.
use core::ffi::{CStr, c_char, c_void};
use obs_output::ffi::output_video_getter::obs_output_video;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

const SLOTS: u32 = 10;
const ENCODED: u32 = 1 << 2;
const ALL_SLOTS: u32 = (1 << SLOTS) - 1;

type Getter = unsafe extern "C" fn(*const c_void) -> *mut c_void;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_obs_output_video(output: *const c_void) -> *mut c_void;
    fn oracle_output_video_new(
        flags: u32,
        raw: bool,
        occupied: u32,
        video: u32,
        media: u32,
        fps_override: u32,
        poison: bool,
    ) -> *mut c_void;
    fn oracle_output_video_free(handle: *mut c_void);
    fn oracle_output_video_run(
        handle: *mut c_void,
        getter: Getter,
        get_log: GetLog,
        set_log: SetLog,
        report: *mut u64,
        message: *mut c_char,
        message_size: usize,
    );
}

#[derive(Clone, Copy, Debug)]
struct Case {
    flags: u32,
    raw: bool,
    occupied: u32,
    video: u32,
    media: u32,
    fps_override: u32,
    poison: bool,
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: uniquely owned oracle fixture (or NULL, which free accepts).
        unsafe { oracle_output_video_free(self.0) }
    }
}

type Report = ([u64; 4], String);

fn run(fixture: &Fixture, getter: Getter) -> Report {
    let mut report = [0u64; 4];
    let mut message = [0 as c_char; 256];
    // SAFETY: a live exclusive fixture or NULL; the run restores the log
    // handler, writes four report entries and a NUL-terminated message.
    unsafe {
        oracle_output_video_run(
            fixture.0,
            getter,
            base_get_log_handler,
            base_set_log_handler,
            report.as_mut_ptr(),
            message.as_mut_ptr(),
            message.len(),
        );
    }
    // SAFETY: snprintf always terminates within the buffer.
    let text = unsafe { CStr::from_ptr(message.as_ptr()) };
    (report, text.to_string_lossy().into_owned())
}

/// The characterized result: identity code, log count, level and message.
fn expected(case: Option<Case>) -> Report {
    let Some(c) = case else {
        return (
            [0, 1, 400, 1],
            "obs_output_video: Null 'output' parameter".into(),
        );
    };
    if c.flags & ENCODED == 0 {
        return ([u64::from(c.raw), 0, 0, 1], String::new());
    }
    let Some(first) = (0..SLOTS).find(|i| c.occupied & (1 << i) != 0) else {
        return (
            [0, 1, 400, 1],
            "obs_encoder_video: Null 'encoder' parameter".into(),
        );
    };
    let bit = 1 << first;
    if c.video & bit == 0 {
        return (
            [0, 1, 200, 1],
            "obs_encoder_set_video: encoder 'video-%s' is not a video encoder".into(),
        );
    }
    let identity = if c.fps_override & bit != 0 {
        2 + u64::from(SLOTS + first)
    } else if c.media & bit != 0 {
        2 + u64::from(first)
    } else {
        0
    };
    ([identity, 0, 0, 1], String::new())
}

fn check(case: Option<Case>) {
    let fixture = Fixture(match case {
        // SAFETY: the oracle allocates and initializes real private layouts.
        Some(c) => unsafe {
            oracle_output_video_new(
                c.flags,
                c.raw,
                c.occupied,
                c.video,
                c.media,
                c.fps_override,
                c.poison,
            )
        },
        None => core::ptr::null_mut(),
    });
    if case.is_some() {
        assert!(!fixture.0.is_null());
    }
    let original = run(&fixture, oracle_obs_output_video);
    let rust = run(&fixture, obs_output_video);
    assert_eq!(rust, original, "{case:?}");
    assert_eq!(rust, expected(case), "{case:?}");
}

#[test]
fn characterized_edges_match_original_c() {
    if !super::isolated(concat!(
        module_path!(),
        "::characterized_edges_match_original_c"
    )) {
        return;
    }
    check(None);
    // Raw outputs never follow the (unreadable) slots, whatever other bits are set.
    for flags in [0, 1, 2 | 8, u32::MAX ^ ENCODED] {
        for raw in [false, true] {
            check(Some(Case {
                flags,
                raw,
                occupied: 0,
                video: 0,
                media: 0,
                fps_override: 0,
                poison: true,
            }));
        }
    }
    // Encoded outputs: every single occupied slot (gaps up to the final slot),
    // no slot, both encoder types and the media/override selection.
    for flags in [ENCODED, ENCODED | 2, ENCODED | 1, u32::MAX] {
        for raw in [false, true] {
            for occupied in (0..SLOTS)
                .map(|i| 1 << i)
                .chain([0, ALL_SLOTS, 0b10_0000_0010])
            {
                for video in [0, ALL_SLOTS] {
                    for media in [0, ALL_SLOTS] {
                        for fps_override in [0, ALL_SLOTS] {
                            check(Some(Case {
                                flags,
                                raw,
                                occupied,
                                video,
                                media,
                                fps_override,
                                poison: false,
                            }));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn randomized_routing_matches_original_c() {
    if !super::isolated(concat!(
        module_path!(),
        "::randomized_routing_matches_original_c"
    )) {
        return;
    }
    let mut runner = proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..Default::default()
    });
    runner
        .run(
            &(
                any::<u32>(),
                any::<bool>(),
                0..=ALL_SLOTS,
                0..=ALL_SLOTS,
                0..=ALL_SLOTS,
                0..=ALL_SLOTS,
            ),
            |(flags, raw, occupied, video, media, fps_override)| {
                check(Some(Case {
                    flags,
                    raw,
                    occupied,
                    video,
                    media,
                    fps_override,
                    // Unreadable slots only where the raw branch must skip them.
                    poison: flags & ENCODED == 0 && occupied & 1 != 0,
                }));
                Ok(())
            },
        )
        .unwrap();
}
