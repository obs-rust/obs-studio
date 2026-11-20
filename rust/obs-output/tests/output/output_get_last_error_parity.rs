//! Tier 3: the unchanged extracted `libobs/obs-output-get-last-error.c.h`
//! against the Rust ABI, both over one real private output and encoder
//! layout. The Rust side reads through the production field adapters, which
//! are checked to address exactly the fields the original reads. Mutants of
//! the query run through the same harness and must be caught. No
//! intentional differences; no input exclusions.
#![allow(unsafe_code)]
use core::ffi::{c_char, c_void};
use obs_output::ffi::encoder_last_error::LastErrorFields as EncoderFields;
use obs_output::ffi::output_get_last_error::obs_output_get_last_error;
use obs_output::output_slots::{MAX_OUTPUT_AUDIO_ENCODERS, MAX_OUTPUT_VIDEO_ENCODERS};
use proptest::prelude::*;

const V: usize = MAX_OUTPUT_VIDEO_ENCODERS;
const A: usize = MAX_OUTPUT_AUDIO_ENCODERS;
const FIELDS: usize = 1 + V + A;
/// NULL, "same error", "", non-UTF-8 bytes (the oracle's `enum ogle_kind`).
const KINDS: u8 = 4;
const SAME: u8 = 1;
const EMPTY: u8 = 2;

type Getter = unsafe extern "C" fn(*mut c_void) -> *const c_char;
unsafe extern "C" {
    fn oracle_obs_output_get_last_error(output: *mut c_void) -> *const c_char;
    fn oracle_output_get_last_error_new(kinds: *const u8, video: u32, audio: u32) -> *mut c_void;
    fn oracle_output_get_last_error_storage(fixture: *const c_void, field: usize) -> *const c_char;
    fn oracle_output_get_last_error_unchanged(fixture: *const c_void) -> bool;
    fn oracle_output_get_last_error_free(fixture: *mut c_void);
    fn oracle_output_get_last_error_layout(out: *mut usize);
    fn oracle_output_get_last_error_adapters(fixture: *mut c_void) -> bool;
    fn oracle_output_get_last_error_null_log(getter: Getter) -> bool;
    // The production adapters the shim reads through, for the mutants.
    fn obs_output_last_error_fields(output: *mut c_void) -> *mut *mut c_char;
    fn obs_output_video_encoder_getter_at(output: *const c_void, idx: usize) -> *mut c_void;
    fn obs_output_encoder_getters_audio_at(output: *const c_void, idx: usize) -> *mut c_void;
    fn obs_encoder_last_error_fields(encoder: *mut c_void) -> EncoderFields;
}

struct Fixture(*mut c_void);
impl Fixture {
    fn new(kinds: &[u8; FIELDS], video: u32, audio: u32) -> Self {
        // SAFETY: kinds has one entry per field; the oracle allocates the
        // real output and encoder layouts with independent message storage.
        let fixture = unsafe { oracle_output_get_last_error_new(kinds.as_ptr(), video, audio) };
        assert!(!fixture.is_null());
        Self(fixture)
    }
    /// The field whose storage `result` is, if any: identity, never a copy.
    fn field_of(&self, result: *const c_char) -> Option<usize> {
        // SAFETY: the live fixture; every field index is in range.
        (0..FIELDS).find(|&f| unsafe { oracle_output_get_last_error_storage(self.0, f) } == result)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: the fixture owns all of its storage; nothing borrows it now.
        unsafe { oracle_output_get_last_error_free(self.0) }
    }
}

/// The field the original returns for this input, or `None` when it is NULL.
fn outcome(getter: Getter, kinds: &[u8; FIELDS], video: u32, audio: u32) -> Option<usize> {
    let fixture = Fixture::new(kinds, video, audio);
    // SAFETY: a live, non-NULL fixture whose slots hold its own encoders.
    let result = unsafe { getter(fixture.0) };
    // SAFETY: as above; the query must leave every field and byte unchanged.
    assert!(unsafe { oracle_output_get_last_error_unchanged(fixture.0) });
    let field = fixture.field_of(result);
    assert!(
        result.is_null() || field.is_some(),
        "result is not borrowed"
    );
    field
}

fn check(kinds: &[u8; FIELDS], video: u32, audio: u32) {
    let fixture = Fixture::new(kinds, video, audio);
    // SAFETY: the real-layout adapters of the live fixture.
    assert!(unsafe { oracle_output_get_last_error_adapters(fixture.0) });
    drop(fixture);
    for _ in 0..2 {
        assert_eq!(
            outcome(obs_output_get_last_error, kinds, video, audio),
            outcome(oracle_obs_output_get_last_error, kinds, video, audio),
            "kinds {kinds:?}, video {video:#x}, audio {audio:#x}"
        );
    }
}

/// Characterized shapes: every kind at every field alone, in each slot
/// arrangement, plus the C test's "all equal bytes" and "holes" layouts.
fn characterized() -> Vec<([u8; FIELDS], u32, u32)> {
    let masks = [
        (0, 0),
        (u32::MAX, u32::MAX),
        (1 << (V - 1), 1 << (A - 1)),
        (0b10_1010, 0b01_0101),
    ];
    let mut cases = Vec::new();
    for (video, audio) in masks {
        for field in 0..FIELDS {
            for kind in 0..KINDS {
                let mut kinds = [0; FIELDS];
                kinds[field] = kind;
                cases.push((kinds, video, audio));
            }
        }
        cases.push(([SAME; FIELDS], video, audio));
        let mut empty_first = [SAME; FIELDS];
        empty_first[0] = 0;
        empty_first[1] = EMPTY;
        cases.push((empty_first, video, audio));
        let mut audio_only = [0; FIELDS];
        audio_only[1 + V..].fill(SAME);
        cases.push((audio_only, video, audio));
    }
    cases
}

#[test]
fn slot_counts_match_the_c_build() {
    let mut layout = [0usize; 3];
    // SAFETY: the oracle writes exactly three values.
    unsafe { oracle_output_get_last_error_layout(layout.as_mut_ptr()) };
    assert_eq!(layout, [V, A, FIELDS]);
}

#[test]
fn characterized_edges() {
    for (kinds, video, audio) in characterized() {
        check(&kinds, video, audio);
    }
}

#[test]
fn exact_null_diagnostic_without_allocation() {
    if !super::isolated(concat!(
        module_path!(),
        "::exact_null_diagnostic_without_allocation"
    )) {
        return;
    }
    // SAFETY: the isolated child owns the log handler and allocator counter.
    unsafe {
        assert!(oracle_output_get_last_error_null_log(
            oracle_obs_output_get_last_error
        ));
        assert!(oracle_output_get_last_error_null_log(
            obs_output_get_last_error
        ));
        let fixture = Fixture::new(&[SAME; FIELDS], u32::MAX, u32::MAX);
        let baseline = obs_util::ffi::bmem::bnum_allocs();
        assert!(!obs_output_get_last_error(fixture.0).is_null());
        assert_eq!(obs_util::ffi::bmem::bnum_allocs(), baseline);
    }
}

#[derive(Clone, Copy, Debug)]
enum Mutant {
    SkipsOwnError,
    AudioBeforeVideo,
    DescendingSlots,
    EmptyIsMissing,
    Copies,
}

thread_local! {
    static COPY: core::cell::RefCell<[u8; 16]> = const { core::cell::RefCell::new([0; 16]) };
}

/// A wrong query over the same production adapters.
unsafe fn mutant(kind: Mutant, output: *mut c_void) -> *const c_char {
    // SAFETY: the caller passes a live fixture; every index is in range.
    unsafe {
        let error = |encoder: *mut c_void| -> *const c_char {
            if encoder.is_null() {
                core::ptr::null()
            } else {
                *obs_encoder_last_error_fields(encoder).message
            }
        };
        let present = |message: *const c_char| {
            !message.is_null() && !(matches!(kind, Mutant::EmptyIsMissing) && *message == 0)
        };
        let own = *obs_output_last_error_fields(output);
        if !matches!(kind, Mutant::SkipsOwnError) && present(own) {
            return own;
        }
        let video = |idx| error(obs_output_video_encoder_getter_at(output, idx));
        let audio = |idx| error(obs_output_encoder_getters_audio_at(output, idx));
        let mut order: Vec<*const c_char> = Vec::new();
        let videos: Vec<_> = if matches!(kind, Mutant::DescendingSlots) {
            (0..V).rev().map(video).collect()
        } else {
            (0..V).map(video).collect()
        };
        let audios: Vec<_> = (0..A).map(audio).collect();
        if matches!(kind, Mutant::AudioBeforeVideo) {
            order.extend(audios);
            order.extend(videos);
        } else {
            order.extend(videos);
            order.extend(audios);
        }
        let found = order
            .into_iter()
            .find(|&m| present(m))
            .unwrap_or(core::ptr::null());
        if matches!(kind, Mutant::Copies) && !found.is_null() {
            return COPY.with(|copy| {
                let mut copy = copy.borrow_mut();
                let bytes = core::ffi::CStr::from_ptr(found).to_bytes_with_nul();
                copy[..bytes.len()].copy_from_slice(bytes);
                copy.as_ptr().cast()
            });
        }
        found
    }
}

macro_rules! mutant_getter {
    ($name:ident, $kind:expr) => {
        unsafe extern "C" fn $name(output: *mut c_void) -> *const c_char {
            // SAFETY: forwarded from the harness, which passes a live fixture.
            unsafe { mutant($kind, output) }
        }
    };
}
mutant_getter!(skips_own_error, Mutant::SkipsOwnError);
mutant_getter!(audio_before_video, Mutant::AudioBeforeVideo);
mutant_getter!(descending_slots, Mutant::DescendingSlots);
mutant_getter!(empty_is_missing, Mutant::EmptyIsMissing);
mutant_getter!(copies, Mutant::Copies);

#[test]
fn mutants_are_caught() {
    let mutants: [(&str, Getter); 5] = [
        ("skips own error", skips_own_error),
        ("audio before video", audio_before_video),
        ("descending slots", descending_slots),
        ("empty is missing", empty_is_missing),
        ("copies", copies),
    ];
    for (name, getter) in mutants {
        let caught = characterized().iter().any(|(kinds, video, audio)| {
            std::panic::catch_unwind(|| {
                outcome(getter, kinds, *video, *audio)
                    != outcome(oracle_obs_output_get_last_error, kinds, *video, *audio)
            })
            .unwrap_or(true)
        });
        assert!(caught, "mutant {name} survived the characterized cases");
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn random_fields_and_slots(kinds in prop::collection::vec(0..KINDS, FIELDS), video in any::<u32>(), audio in any::<u32>()) {
        let kinds: [u8; FIELDS] = kinds.try_into().unwrap();
        check(&kinds, video, audio);
    }
}
