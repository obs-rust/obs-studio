//! Tier 3: the original C getters and the Rust exports on noise-filled
//! real-header sources, with exact diagnostics, and a reader on another
//! thread while the audio buffer mutex is held. No intentional differences
//! or excluded inputs.
use core::ffi::{CStr, c_char, c_void};
use core::mem::size_of;
use obs_source::audio_buffer_getters::{MAX_AUDIO_CHANNELS, MAX_AUDIO_MIXES, Mix};
use obs_source::ffi::source_audio_buffer_getters::{
    obs_source_get_audio_mix, obs_source_get_audio_timestamp,
};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

type TimestampQuery = unsafe extern "C" fn(*const c_void) -> u64;
type MixQuery = unsafe extern "C" fn(*const c_void, *mut c_void);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_source_audio_buffer_layout(layout: *mut usize);
    fn oracle_source_audio_timestamp_check(
        query: TimestampQuery,
        get: GetLog,
        set: SetLog,
        present: bool,
        noise: u8,
        ts: u64,
        answer: *mut u64,
        message: *mut c_char,
        message_size: usize,
    ) -> bool;
    fn oracle_source_audio_mix_check(
        query: MixQuery,
        get: GetLog,
        set: SetLog,
        present: bool,
        audio_present: bool,
        noise: u8,
        copied: *mut bool,
        untouched: *mut bool,
        message: *mut c_char,
        message_size: usize,
    ) -> bool;
    fn oracle_obs_source_get_audio_timestamp(source: *const c_void) -> u64;
    fn oracle_source_audio_buffer_new(ts: u64) -> *mut c_void;
    fn oracle_source_audio_buffer_free(source: *mut c_void);
    fn oracle_source_audio_buffer_lock(source: *mut c_void);
    fn oracle_source_audio_buffer_unlock_with(source: *mut c_void, ts: u64);
    fn obs_source_audio_ts_field(source: *const c_void) -> *const u64;
}

fn message(buffer: &[c_char; 256]) -> String {
    // SAFETY: the oracle wrote a NUL-terminated line into the buffer.
    unsafe { CStr::from_ptr(buffer.as_ptr()) }
        .to_str()
        .unwrap()
        .to_owned()
}

/// The answer and the original's log line, after checking both sides agree
/// on the answers, their logs, every byte outside the mutex and the mutex
/// left unlocked.
fn timestamp_with(
    query: TimestampQuery,
    present: bool,
    noise: u8,
    ts: u64,
) -> Option<(u64, String)> {
    let (mut answer, mut line) = (0, [0 as c_char; 256]);
    // SAFETY: the isolated child owns the log handler. C builds live sources,
    // runs both getters, compares them and restores logging.
    let equal = unsafe {
        oracle_source_audio_timestamp_check(
            query,
            base_get_log_handler,
            base_set_log_handler,
            present,
            noise,
            ts,
            &raw mut answer,
            line.as_mut_ptr(),
            line.len(),
        )
    };
    equal.then(|| (answer, message(&line)))
}

fn timestamp(present: bool, noise: u8, ts: u64) -> (u64, String) {
    timestamp_with(obs_source_get_audio_timestamp, present, noise, ts)
        .unwrap_or_else(|| panic!("present={present}, noise={noise:#x}, ts={ts:#x}"))
}

/// Whether the mix is the source's buffers, whether it is still the fill,
/// and the original's log line, after checking both sides agree on the mix,
/// their logs and every source byte.
fn mix_with(
    query: MixQuery,
    present: bool,
    audio_present: bool,
    noise: u8,
) -> Option<(bool, bool, String)> {
    let (mut copied, mut untouched, mut line) = (false, false, [0 as c_char; 256]);
    // SAFETY: as for the timestamp; the mixes are C's own.
    let equal = unsafe {
        oracle_source_audio_mix_check(
            query,
            base_get_log_handler,
            base_set_log_handler,
            present,
            audio_present,
            noise,
            &raw mut copied,
            &raw mut untouched,
            line.as_mut_ptr(),
            line.len(),
        )
    };
    equal.then(|| (copied, untouched, message(&line)))
}

fn mix(present: bool, audio_present: bool, noise: u8) -> (bool, bool, String) {
    mix_with(obs_source_get_audio_mix, present, audio_present, noise).unwrap_or_else(|| {
        panic!("present={present}, audio_present={audio_present}, noise={noise:#x}")
    })
}

/// The expected outcome of the mix, pinned on top of the comparison.
fn expected_mix(present: bool, audio_present: bool) -> (bool, bool, String) {
    match (present, audio_present) {
        (false, _) => (
            false,
            true,
            "obs_source_get_audio_mix: Null 'source' parameter".into(),
        ),
        (true, false) => (false, true, "audio: Null 'audio' parameter".into()),
        (true, true) => (true, false, String::new()),
    }
}

/// Mutants of the shims: each must disagree with the original somewhere.
unsafe extern "C" fn ts_off_by_one(source: *const c_void) -> u64 {
    // SAFETY: NULL or a live oracle source.
    let ts = unsafe { obs_source_get_audio_timestamp(source) };
    if source.is_null() {
        ts
    } else {
        ts.wrapping_add(1)
    }
}
unsafe extern "C" fn ts_silent_null(source: *const c_void) -> u64 {
    if source.is_null() {
        return 0;
    }
    // SAFETY: a live oracle source.
    unsafe { obs_source_get_audio_timestamp(source) }
}
unsafe extern "C" fn ts_unlocked(source: *const c_void) -> u64 {
    if source.is_null() {
        // SAFETY: NULL logs as the real export does.
        return unsafe { obs_source_get_audio_timestamp(source) };
    }
    // SAFETY: a live oracle source; the mutant reads without the mutex.
    unsafe { obs_source_audio_ts_field(source).read() }
}
unsafe extern "C" fn mix_drops_last(source: *const c_void, audio: *mut c_void) {
    let last = audio.cast::<Mix<*mut f32>>();
    // SAFETY: NULL or the oracle's own mix; the last row is kept as it was.
    unsafe {
        let kept = last.as_ref().map(|mix| mix[MAX_AUDIO_MIXES - 1]);
        obs_source_get_audio_mix(source, audio);
        if let (Some(kept), Some(mix)) = (kept, last.as_mut()) {
            mix[MAX_AUDIO_MIXES - 1] = kept;
        }
    }
}
unsafe extern "C" fn mix_silent_null_audio(source: *const c_void, audio: *mut c_void) {
    if !source.is_null() && audio.is_null() {
        return;
    }
    // SAFETY: NULL or live oracle arguments.
    unsafe { obs_source_get_audio_mix(source, audio) }
}

#[test]
fn audio_mix_layout_matches_the_real_header() {
    let mut layout = [0usize; 4];
    // SAFETY: the oracle writes exactly four entries.
    unsafe { oracle_source_audio_buffer_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Mix<*mut f32>>(),
            MAX_AUDIO_MIXES,
            MAX_AUDIO_CHANNELS,
            size_of::<Mix<*mut f32>>()
        ]
    );
}

#[test]
fn audio_buffer_getters_match_c() {
    if !crate::output::isolated(concat!(module_path!(), "::audio_buffer_getters_match_c")) {
        return;
    }
    let values = [0, 1, 48_000, 0x0123_4567_89ab_cdef, u64::MAX];
    for noise in [0, 0x5a, 0xff] {
        // NULL is 0 whatever the bytes would have held.
        assert_eq!(
            timestamp(false, noise, 7),
            (
                0,
                "obs_source_get_audio_timestamp: Null 'source' parameter".into()
            )
        );
        for ts in values {
            assert_eq!(timestamp(true, noise, ts), (ts, String::new()));
        }
        for present in [false, true] {
            for audio_present in [false, true] {
                assert_eq!(
                    mix(present, audio_present, noise),
                    expected_mix(present, audio_present)
                );
            }
        }
    }
    let ts_mutants: [TimestampQuery; 2] = [ts_off_by_one, ts_silent_null];
    for mutant in ts_mutants {
        let caught = [(false, 7), (true, 0), (true, u64::MAX)]
            .into_iter()
            .any(|(present, ts)| timestamp_with(mutant, present, 0x5a, ts).is_none());
        assert!(caught, "a timestamp mutant agreed with the original C");
    }
    let mix_mutants: [MixQuery; 2] = [mix_drops_last, mix_silent_null_audio];
    for mutant in mix_mutants {
        let caught = [(false, true), (true, false), (true, true)]
            .into_iter()
            .flat_map(|cases| [0u8, 1, 2].map(|noise| (cases, noise)))
            .any(|((present, audio_present), noise)| {
                mix_with(mutant, present, audio_present, noise).is_none()
            });
        assert!(caught, "a mix mutant agreed with the original C");
    }
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(any::<bool>(), any::<bool>(), any::<u8>(), any::<u64>()),
            |(present, audio_present, noise, ts)| {
                prop_assert_eq!(
                    timestamp(present, noise, ts).0,
                    if present { ts } else { 0 }
                );
                prop_assert_eq!(
                    mix(present, audio_present, noise),
                    expected_mix(present, audio_present)
                );
                Ok(())
            },
        )
        .unwrap();
}

/// Whether a reader on another thread waits while this one holds the real
/// audio_buf_mutex, and returns what was written before the unlock.
fn waits(query: TimestampQuery) -> bool {
    // SAFETY: the oracle allocates a zeroed source with a live mutex.
    let source = unsafe { oracle_source_audio_buffer_new(1000) };
    assert!(!source.is_null());
    // SAFETY: the live source's mutex, released below.
    unsafe { oracle_source_audio_buffer_lock(source) };
    let (started, done) = (
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicBool::new(false)),
    );
    let address = source as usize;
    let reader = {
        let (started, done) = (Arc::clone(&started), Arc::clone(&done));
        std::thread::spawn(move || {
            started.store(true, Ordering::SeqCst);
            // SAFETY: the source outlives the join below.
            let ts = unsafe { query(address as *const c_void) };
            done.store(true, Ordering::SeqCst);
            ts
        })
    };
    while !started.load(Ordering::SeqCst) {
        std::hint::spin_loop();
    }
    std::thread::sleep(Duration::from_millis(100));
    let waited = !done.load(Ordering::SeqCst);
    // SAFETY: writes audio_ts under the held mutex, then releases it.
    unsafe { oracle_source_audio_buffer_unlock_with(source, 2000) };
    let ts = reader.join().unwrap();
    // SAFETY: the reader has finished; the fixture is freed once.
    unsafe { oracle_source_audio_buffer_free(source) };
    waited && ts == 2000
}

#[test]
fn timestamp_waits_for_the_audio_buffer_mutex_like_c() {
    assert!(waits(oracle_obs_source_get_audio_timestamp));
    assert!(waits(obs_source_get_audio_timestamp));
    assert!(
        !waits(ts_unlocked),
        "the unlocked mutant did not race ahead"
    );
}
