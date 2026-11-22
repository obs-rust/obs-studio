//! Tier 3: the original extracted `obs_get_video_frame_counts()` versus the
//! shim on one recording fixture core (oracle/get_video_frame_counts.c),
//! through the real-named main mix, mutex, list, field and counter adapters.
//! Compared: both outputs (sentinels where nothing is written, one slot for
//! both); the trace of every core read, lock and unlock, every field the mix
//! rule asks about with its answer and every counter read with its mix (so
//! the order, the reads the rule skips and the lock bracket all show), with
//! its checks that every read runs under the lock and no output is written
//! before the unlock; the mutex released; and the untouched bytes of the
//! canvas, the list, the views and every mix. Cases: no main canvas, a
//! canvas without a mix, a listed or unlisted main mix, empty, single, many
//! and duplicate lists, mixes skipped at each rule step, NULL and shared
//! outputs, and sums that wrap.
//!
//! Excluded: an uninitialized core, a dead main canvas, a NULL or dead
//! listed mix and a concurrent list change, outside the original's contract.
//! No intentional differences. Mutants show the probe catches each broken
//! step.
use core::ffi::{CStr, c_char};
use obs_core::ffi::get_video_frame_counts::obs_get_video_frame_counts;
use proptest::prelude::*;

const POOL: usize = 8;
const SENTINEL: u32 = 0xdead_beef;

type Counts = unsafe extern "C" fn(*mut u32, *mut u32);

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct FixtureMix {
    encoder_only: u8,
    view: u8,
    video: u8,
    total: u32,
    skipped: u32,
}

#[repr(C)]
struct Case {
    list: *const u8,
    count: usize,
    mixes: *const FixtureMix,
    main: u8,
    main_index: u8,
    outputs: u8,
}

unsafe extern "C" {
    fn oracle_get_video_frame_counts_probe(candidate: Counts, case: *const Case) -> u32;
    fn oracle_get_video_frame_counts_original(
        case: *const Case,
        out: *mut u32,
        trace: *mut c_char,
        trace_size: usize,
    ) -> bool;
    fn oracle_get_video_frame_counts_adapters() -> bool;
}

/// No main canvas, a canvas without a mix, or a canvas whose mix is
/// `pool[index]`.
#[derive(Debug, Clone, Copy)]
enum Main {
    NoCanvas,
    NoMix,
    Mix(u8),
}

fn case(list: &[u8], mixes: &[FixtureMix; POOL], main: Main, outputs: u8) -> Case {
    let (main, main_index) = match main {
        Main::NoCanvas => (0, 0),
        Main::NoMix => (1, 0),
        Main::Mix(index) => (2, index),
    };
    Case {
        list: list.as_ptr(),
        count: list.len(),
        mixes: mixes.as_ptr(),
        main,
        main_index,
        outputs,
    }
}

fn probe(
    candidate: Counts,
    list: &[u8],
    mixes: &[FixtureMix; POOL],
    main: Main,
    outputs: u8,
) -> u32 {
    let case = case(list, mixes, main, outputs);
    // SAFETY: the oracle owns the fixture core and mixes every adapter
    // reaches; the list and mixes outlive the call.
    unsafe { oracle_get_video_frame_counts_probe(candidate, &case) }
}

/// The original's outputs and trace.
fn original(
    list: &[u8],
    mixes: &[FixtureMix; POOL],
    main: Main,
    outputs: u8,
) -> ([u32; 2], String) {
    let case = case(list, mixes, main, outputs);
    let mut out = [0_u32; 2];
    let mut trace = [0 as c_char; 512];
    // SAFETY: the buffers are writable for their sizes; the oracle writes
    // NUL-terminated text.
    let untouched = unsafe {
        oracle_get_video_frame_counts_original(
            &case,
            out.as_mut_ptr(),
            trace.as_mut_ptr(),
            trace.len(),
        )
    };
    assert!(untouched, "the original changed the fixture");
    // SAFETY: NUL-terminated by the oracle.
    let text = unsafe { CStr::from_ptr(trace.as_ptr()) }
        .to_str()
        .unwrap()
        .to_owned();
    (out, text)
}

const MAIN_VIEW: u8 = 0;
const OTHER_VIEW: u8 = 1;

fn mix(encoder_only: bool, view: u8, video: Option<(u32, u32)>) -> FixtureMix {
    let (total, skipped) = video.unwrap_or((0, 0));
    FixtureMix {
        encoder_only: u8::from(encoder_only),
        view,
        video: u8::from(video.is_some()),
        total,
        skipped,
    }
}

/// Pool: 0 the main mix, 1 an encoder-only mix of its view, 2 an auxiliary
/// canvas's mix, 3 an encoder-only mix of another view, 4 an encoder-only
/// mix of the main view without video, 5 a plain mix of the main view.
fn pool() -> [FixtureMix; POOL] {
    let mut pool = [FixtureMix::default(); POOL];
    pool[0] = mix(false, MAIN_VIEW, Some((57, 11)));
    pool[1] = mix(true, MAIN_VIEW, Some((30, 8)));
    pool[2] = mix(false, OTHER_VIEW, Some((200, 90)));
    pool[3] = mix(true, OTHER_VIEW, Some((150, 70)));
    pool[4] = mix(true, MAIN_VIEW, None);
    pool[5] = mix(false, MAIN_VIEW, Some((100, 100)));
    pool
}

#[test]
fn real_adapters_read_the_real_fields() {
    // SAFETY: the oracle arms and frees its own fixture.
    assert!(unsafe { oracle_get_video_frame_counts_adapters() });
}

#[test]
fn original_c_is_the_characterized_walk() {
    let pool = pool();
    // No main canvas, or a canvas without a mix: zero, no lock.
    assert_eq!(
        original(&[0, 1], &pool, Main::NoCanvas, 0),
        ([0, 0], "o".into())
    );
    assert_eq!(
        original(&[0, 1], &pool, Main::NoMix, 0),
        ([0, 0], "oo".into())
    );
    // An empty list reads only its count.
    assert_eq!(
        original(&[], &pool, Main::Mix(0), 0),
        ([0, 0], "oooLooU".into())
    );
    // The main mix counts after its video check; an encoder-only mix of the
    // main view is asked every field; a plain mix only encoder_only_mix; an
    // encoder-only mix of another view stops at the view; one without video
    // at the video.
    assert_eq!(
        original(&[0, 1, 5, 3, 4], &pool, Main::Mix(0), 0),
        (
            [87, 19],
            "oooLood1t0s0ooe1v1d1t1s1ooe0ooe1v0ooe1v1d0ooU".into()
        )
    );
    // NULL outputs are skipped; one slot for both keeps skipped.
    assert_eq!(original(&[0], &pool, Main::Mix(0), 1).0, [SENTINEL, 11]);
    assert_eq!(original(&[0], &pool, Main::Mix(0), 2).0, [57, SENTINEL]);
    assert_eq!(
        original(&[0], &pool, Main::Mix(0), 3).0,
        [SENTINEL, SENTINEL]
    );
    assert_eq!(original(&[0], &pool, Main::Mix(0), 4).0, [11, SENTINEL]);
}

#[test]
fn characterized_edges() {
    let shim: Counts = obs_get_video_frame_counts;
    let mut pool = pool();
    pool[6] = mix(true, MAIN_VIEW, Some((u32::MAX, u32::MAX)));
    pool[7] = mix(true, MAIN_VIEW, Some((2, 0)));
    let lists: [&[u8]; 9] = [
        &[],
        &[0],
        &[1],
        &[0, 1, 2, 3, 4, 5],
        &[5, 4, 3, 2, 1, 0],
        &[0, 0, 1, 1],
        &[2, 3],
        &[0, 6, 7, 6],
        &[6, 6, 6, 7, 0, 1, 2, 3, 4, 5, 6, 7, 0, 1, 2, 3],
    ];
    let mains = [
        Main::NoCanvas,
        Main::NoMix,
        Main::Mix(0),
        Main::Mix(1),
        Main::Mix(2),
        Main::Mix(4),
        Main::Mix(6),
    ];
    for list in lists {
        for main in mains {
            for outputs in 0..=4 {
                assert_eq!(
                    probe(shim, list, &pool, main, outputs),
                    0,
                    "{list:?} {main:?} {outputs}"
                );
            }
        }
    }
}

fn fixture_mix() -> impl Strategy<Value = FixtureMix> {
    let counter = prop_oneof![Just(0), Just(u32::MAX), 0..4_u32, any::<u32>()];
    (
        any::<bool>(),
        0..3_u8,
        any::<bool>(),
        counter.clone(),
        counter,
    )
        .prop_map(|(encoder_only, view, video, total, skipped)| FixtureMix {
            encoder_only: u8::from(encoder_only),
            view,
            video: u8::from(video),
            total,
            skipped,
        })
}

fn main_strategy() -> impl Strategy<Value = Main> {
    prop_oneof![
        1 => Just(Main::NoCanvas),
        1 => Just(Main::NoMix),
        6 => (0..POOL as u8).prop_map(Main::Mix),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn any_core_matches_original_c(
        list in proptest::collection::vec(0..POOL as u8, 0..=16),
        mixes in proptest::array::uniform8(fixture_mix()),
        main in main_strategy(),
        outputs in 0..=4_u8,
    ) {
        prop_assert_eq!(probe(obs_get_video_frame_counts, &list, &mixes, main, outputs), 0);
    }
}
