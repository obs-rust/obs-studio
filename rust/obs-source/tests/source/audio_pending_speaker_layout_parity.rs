//! Tier 3: the original C getters and the Rust exports on noise-filled
//! real-header sources, with exact diagnostics. No intentional differences
//! or excluded inputs.
use core::ffi::{CStr, c_char, c_int, c_void};
use obs_source::ffi::source_audio_pending_speaker_layout::{
    obs_source_audio_pending, obs_source_get_speaker_layout,
};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type PendingQuery = unsafe extern "C" fn(*const c_void) -> bool;
type LayoutQuery = unsafe extern "C" fn(*mut c_void) -> c_int;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_source_audio_pending_check(
        query: PendingQuery,
        get: GetLog,
        set: SetLog,
        present: bool,
        noise: u8,
        removed: bool,
        flags: u32,
        pending: bool,
        answer: *mut bool,
        message: *mut c_char,
        message_size: usize,
    ) -> bool;
    fn oracle_source_speaker_layout_check(
        query: LayoutQuery,
        get: GetLog,
        set: SetLog,
        present: bool,
        noise: u8,
        removed: bool,
        speakers: c_int,
        answer: *mut c_int,
        message: *mut c_char,
        message_size: usize,
    ) -> bool;
}

const AUDIO: u32 = 1 << 1;
const COMPOSITE: u32 = 1 << 6;
const FLAGS: [u32; 8] = [
    0,
    1,
    AUDIO,
    COMPOSITE,
    AUDIO | COMPOSITE,
    !(AUDIO | COMPOSITE),
    !AUDIO,
    u32::MAX,
];

/// The answer and the original's log line, after checking both sides agree
/// on the answer, their logs and every source byte.
fn pending_with(
    query: PendingQuery,
    present: bool,
    noise: u8,
    removed: bool,
    flags: u32,
    pending: bool,
) -> Option<(bool, String)> {
    let mut answer = false;
    let mut message = [0 as c_char; 256];
    // SAFETY: the isolated child owns the log handler. C builds live sources,
    // runs both queries, compares results, logs and bytes, and restores
    // logging; the message buffer is as long as passed.
    let equal = unsafe {
        oracle_source_audio_pending_check(
            query,
            base_get_log_handler,
            base_set_log_handler,
            present,
            noise,
            removed,
            flags,
            pending,
            &raw mut answer,
            message.as_mut_ptr(),
            message.len(),
        )
    };
    // SAFETY: snprintf NUL-terminated the buffer.
    let message = unsafe { CStr::from_ptr(message.as_ptr()) };
    equal.then(|| (answer, message.to_str().unwrap().to_owned()))
}

fn layout_with(
    query: LayoutQuery,
    present: bool,
    noise: u8,
    removed: bool,
    speakers: c_int,
) -> Option<(c_int, String)> {
    let mut answer = -1;
    let mut message = [0 as c_char; 256];
    // SAFETY: as in pending_with.
    let equal = unsafe {
        oracle_source_speaker_layout_check(
            query,
            base_get_log_handler,
            base_set_log_handler,
            present,
            noise,
            removed,
            speakers,
            &raw mut answer,
            message.as_mut_ptr(),
            message.len(),
        )
    };
    // SAFETY: snprintf NUL-terminated the buffer.
    let message = unsafe { CStr::from_ptr(message.as_ptr()) };
    equal.then(|| (answer, message.to_str().unwrap().to_owned()))
}

fn pending(present: bool, noise: u8, removed: bool, flags: u32, field: bool) -> (bool, String) {
    pending_with(obs_source_audio_pending, present, noise, removed, flags, field).unwrap_or_else(
        || {
            panic!(
                "present={present}, noise={noise:#x}, removed={removed}, flags={flags:#x}, pending={field}"
            )
        },
    )
}

fn layout(present: bool, noise: u8, removed: bool, speakers: c_int) -> (c_int, String) {
    layout_with(
        obs_source_get_speaker_layout,
        present,
        noise,
        removed,
        speakers,
    )
    .unwrap_or_else(|| {
        panic!("present={present}, noise={noise:#x}, removed={removed}, speakers={speakers}")
    })
}

fn expected_pending(present: bool, removed: bool, flags: u32, field: bool) -> bool {
    !present || removed || flags & (AUDIO | COMPOSITE) == 0 || field
}

/// Mutants of the shims: each must disagree with the original somewhere.
unsafe extern "C" fn pending_silent_null(source: *const c_void) -> bool {
    if source.is_null() {
        return true;
    }
    // SAFETY: a live oracle source.
    unsafe { obs_source_audio_pending(source) }
}
unsafe extern "C" fn pending_null_false(source: *const c_void) -> bool {
    // SAFETY: NULL or a live oracle source.
    let answer = unsafe { obs_source_audio_pending(source) };
    if source.is_null() { false } else { answer }
}
unsafe extern "C" fn pending_negated(source: *const c_void) -> bool {
    // SAFETY: NULL or a live oracle source.
    let answer = unsafe { obs_source_audio_pending(source) };
    if source.is_null() { answer } else { !answer }
}
unsafe extern "C" fn layout_logs_its_own_name(source: *mut c_void) -> c_int {
    if source.is_null() {
        // SAFETY: static strings match the two %s arguments of the format.
        unsafe {
            obs_util_blog(
                400,
                c"%s: Null '%s' parameter".as_ptr(),
                c"obs_source_get_speaker_layout".as_ptr(),
                c"source".as_ptr(),
            );
        }
        return 0;
    }
    // SAFETY: a live oracle source.
    unsafe { obs_source_get_speaker_layout(source) }
}
unsafe extern "C" fn layout_off_by_one(source: *mut c_void) -> c_int {
    // SAFETY: NULL or a live oracle source.
    let answer = unsafe { obs_source_get_speaker_layout(source) };
    if source.is_null() {
        answer
    } else {
        answer.wrapping_add(1)
    }
}

unsafe extern "C" {
    #[link_name = "blog"]
    fn obs_util_blog(level: c_int, format: *const c_char, ...);
}

#[test]
fn audio_pending_and_speaker_layout_match_c() {
    if !crate::output::isolated(concat!(
        module_path!(),
        "::audio_pending_and_speaker_layout_match_c"
    )) {
        return;
    }
    for noise in [0, 0x5a, 0xff] {
        // NULL answers the defaults, whatever the bytes would have held, and
        // logs one line each; the layout's names obs_source_get_audio_channels.
        assert_eq!(
            pending(false, noise, false, AUDIO, false),
            (
                true,
                "obs_source_audio_pending: Null 'source' parameter".to_owned()
            )
        );
        assert_eq!(
            layout(false, noise, false, 2),
            (
                0,
                "obs_source_get_audio_channels: Null 'source' parameter".to_owned()
            )
        );
        for removed in [false, true] {
            for flags in FLAGS {
                for field in [false, true] {
                    assert_eq!(
                        pending(true, noise, removed, flags, field),
                        (expected_pending(true, removed, flags, field), String::new())
                    );
                }
            }
            for speakers in [0, 1, 2, 3, 4, 5, 6, 7, 8, 0x5a, -1, c_int::MAX] {
                assert_eq!(
                    layout(true, noise, removed, speakers),
                    (speakers, String::new())
                );
            }
        }
    }
    let pending_mutants: [PendingQuery; 3] =
        [pending_silent_null, pending_null_false, pending_negated];
    for mutant in pending_mutants {
        let caught = [
            (false, false, AUDIO, false),
            (true, false, AUDIO, false),
            (true, true, AUDIO, false),
            (true, false, 0, false),
        ]
        .into_iter()
        .any(|(present, removed, flags, field)| {
            pending_with(mutant, present, 0x5a, removed, flags, field).is_none()
        });
        assert!(caught, "a pending mutant agreed with the original C");
    }
    let layout_mutants: [LayoutQuery; 2] = [layout_logs_its_own_name, layout_off_by_one];
    for mutant in layout_mutants {
        let caught = [(false, 0), (true, 2)]
            .into_iter()
            .any(|(present, speakers)| {
                layout_with(mutant, present, 0x5a, false, speakers).is_none()
            });
        assert!(caught, "a speaker layout mutant agreed with the original C");
    }
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(
                any::<bool>(),
                any::<u8>(),
                any::<bool>(),
                any::<u32>(),
                any::<bool>(),
                any::<c_int>(),
            ),
            |(present, noise, removed, flags, field, speakers)| {
                prop_assert_eq!(
                    pending(present, noise, removed, flags, field).0,
                    expected_pending(present, removed, flags, field)
                );
                prop_assert_eq!(
                    layout(present, noise, removed, speakers).0,
                    if present { speakers } else { 0 }
                );
                Ok(())
            },
        )
        .unwrap();
}
