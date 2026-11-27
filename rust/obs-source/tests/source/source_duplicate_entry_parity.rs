//! Tier 3: the original C `obs_source_duplicate()` against the Rust export
//! on noise-filled real-header sources: the source passed in, the source a
//! create returns and a scene copy's source. The libobs calls are recording
//! stand-ins for both sides (the lookups, the reference, the scene copy, the
//! settings calls, both creates, the sync offset and volume getters and the
//! filter copies); the decision steps are the build's own. It compares the
//! result, every call in order with the handles and values it was given, the
//! log lines in order with the rest, the copied fields of the new source,
//! that nothing else of any source changed, and the last log line.
use core::cell::Cell;
use core::ffi::{CStr, c_char, c_int, c_long, c_uint, c_void};
use core::ptr::{self, NonNull};
use obs_source::ffi::source_duplicate_entry::obs_source_duplicate;
use obs_source::source_duplicate::{DO_NOT_DUPLICATE, DUP_COPY, DUP_PRIVATE_COPY};
use obs_source::source_duplicate_entry::{Source, duplicate};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Duplicate = unsafe extern "C" fn(*mut c_void, *const c_char, bool) -> *mut c_void;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_sd_run(
        duplicate: Option<Duplicate>,
        get: GetLog,
        set: SetLog,
        present: bool,
        noise: u8,
        source_type: c_int,
        output_flags: u32,
        answers: c_uint,
        named: bool,
        create_private: bool,
        bools: c_uint,
        out: *mut c_long,
        message: *mut c_char,
        message_size: usize,
    ) -> bool;
    fn oracle_sd_source_fields(noise: u8, bools: c_uint, source: bool, out: *mut c_long);
    fn oracle_sd_layout(which: usize) -> usize;
    fn obs_source_duplicate_entry_type(source: *const c_void) -> c_int;
    fn obs_source_duplicate_entry_scene(source: *mut c_void) -> *mut c_void;
    fn obs_source_duplicate_entry_group(source: *mut c_void) -> *mut c_void;
    fn obs_source_duplicate_entry_get_ref(source: *mut c_void) -> *mut c_void;
    fn obs_source_duplicate_entry_scene_duplicate(
        scene: *mut c_void,
        new_name: *const c_char,
        dup_type: c_int,
    ) -> *mut c_void;
    fn obs_source_duplicate_entry_scene_source(scene: *mut c_void) -> *mut c_void;
    fn obs_source_duplicate_entry_output_flags(source: *const c_void) -> u32;
    fn obs_source_duplicate_entry_settings_create() -> *mut c_void;
    fn obs_source_duplicate_entry_settings_apply(settings: *mut c_void, source: *const c_void);
    fn obs_source_duplicate_entry_create(
        source: *const c_void,
        new_name: *const c_char,
        settings: *mut c_void,
        create_private: bool,
    ) -> *mut c_void;
    fn obs_source_duplicate_entry_settings_release(settings: *mut c_void);
    fn obs_source_duplicate_entry_audio_mixers(source: *const c_void) -> u32;
    fn obs_source_duplicate_entry_set_audio_mixers(dst: *mut c_void, value: u32);
    fn obs_source_duplicate_entry_sync_offset(source: *const c_void) -> i64;
    fn obs_source_duplicate_entry_set_sync_offset(dst: *mut c_void, value: i64);
    fn obs_source_duplicate_entry_user_volume(source: *const c_void) -> f32;
    fn obs_source_duplicate_entry_set_user_volume(dst: *mut c_void, value: f32);
    fn obs_source_duplicate_entry_user_muted(source: *const c_void) -> bool;
    fn obs_source_duplicate_entry_set_user_muted(dst: *mut c_void, value: bool);
    fn obs_source_duplicate_entry_volume(source: *const c_void) -> f32;
    fn obs_source_duplicate_entry_set_volume(dst: *mut c_void, value: f32);
    fn obs_source_duplicate_entry_muted(source: *const c_void) -> bool;
    fn obs_source_duplicate_entry_set_muted(dst: *mut c_void, value: bool);
    fn obs_source_duplicate_entry_flags(source: *const c_void) -> u32;
    fn obs_source_duplicate_entry_set_flags(dst: *mut c_void, value: u32);
    fn obs_source_duplicate_entry_private_settings_apply(dst: *mut c_void, source: *const c_void);
    fn obs_source_duplicate_entry_filters(dst: *mut c_void, source: *mut c_void, private: bool);
}

/// The out array's layout (`SD_OUT_*`), the event width (`SD_EVENT_WIDTH`)
/// and the field count (`SD_FIELDS`).
const EVENT_WIDTH: usize = 5;
const EVENTS: usize = 24;
const FIELDS: usize = 7;
const RESULT: usize = 0;
const SOURCE_UNCHANGED: usize = 1;
const CREATED_UNCHANGED: usize = 2;
const COPY_UNCHANGED: usize = 3;
const LOGS: usize = 4;
const EVENT_COUNT: usize = 5;
const OVERFLOW: usize = 6;
const FIELDS_AT: usize = 7;
const HEAD: usize = FIELDS_AT + FIELDS;
const OUT: usize = HEAD + EVENTS * EVENT_WIDTH;
/// Event kinds (`SD_EV_*`).
const EV_SCENE: c_long = 1;
const EV_GROUP: c_long = 2;
const EV_GET_REF: c_long = 3;
const EV_SCENE_DUPLICATE: c_long = 4;
const EV_SCENE_SOURCE: c_long = 5;
const EV_DATA_CREATE: c_long = 6;
const EV_DATA_APPLY: c_long = 7;
const EV_CREATE: c_long = 8;
const EV_CREATE_PRIVATE: c_long = 9;
const EV_DATA_RELEASE: c_long = 10;
const EV_SYNC_OFFSET: c_long = 11;
const EV_GET_VOLUME: c_long = 12;
const EV_DUPLICATE_FILTERS: c_long = 13;
const EV_LOG: c_long = 14;
/// Handle ids (`SD_*`).
const NULL_ID: c_long = -1;
const SOURCE: c_long = 0;
const CREATED: c_long = 1;
const COPY_SOURCE: c_long = 2;
const SCENE: c_long = 10;
const GROUP: c_long = 11;
const SCENE_COPY: c_long = 12;
const SETTINGS: c_long = 20;
const CONTEXT_SETTINGS: c_long = 21;
const SOURCE_PRIVATE: c_long = 22;
const CREATED_PRIVATE: c_long = 23;
const NAME: c_long = 30;
const ID: c_long = 31;
/// What the stand-ins answer (`SD_A_*`).
const A_SCENE: c_uint = 1;
const A_GROUP: c_uint = 2;
const A_REF: c_uint = 4;
const A_SCENE_COPY: c_uint = 8;
const A_CREATED: c_uint = 16;
const ANSWERS: c_uint = 32;
/// Source types.
const TYPES: [c_int; 6] = [0, 1, 2, 3, 4, -1];
const TYPE_FILTER: c_int = 1;
const TYPE_SCENE: c_int = 3;

/// What one call left: the out array and the last log line.
type Seen = (Vec<c_long>, String);

#[derive(Clone, Copy, Debug)]
struct Case {
    present: bool,
    noise: u8,
    source_type: c_int,
    output_flags: u32,
    answers: c_uint,
    named: bool,
    create_private: bool,
    bools: c_uint,
}

fn run(duplicate: Option<Duplicate>, case: Case) -> Seen {
    let mut out: [c_long; OUT] = [0; OUT];
    let mut buffer = [0 as c_char; 128];
    // SAFETY: the isolated child owns the log handler. C builds the fixture,
    // runs the call, records and frees it, and restores logging; both
    // buffers are as long as passed.
    let built = unsafe {
        oracle_sd_run(
            duplicate,
            base_get_log_handler,
            base_set_log_handler,
            case.present,
            case.noise,
            case.source_type,
            case.output_flags,
            case.answers,
            case.named,
            case.create_private,
            case.bools,
            out.as_mut_ptr(),
            buffer.as_mut_ptr(),
            buffer.len(),
        )
    };
    assert!(built, "{case:?}: fixture");
    // SAFETY: the oracle NUL-terminated the buffer.
    let message = unsafe { CStr::from_ptr(buffer.as_ptr()) }
        .to_str()
        .unwrap()
        .to_owned();
    (out.to_vec(), message)
}

/// Both sides alike on `case`; the original's observation.
fn compare_with(duplicate: Duplicate, case: Case) -> Option<Seen> {
    let c = run(None, case);
    let rust = run(Some(duplicate), case);
    (c == rust).then_some(c)
}

fn events(out: &[c_long]) -> Vec<[c_long; EVENT_WIDTH]> {
    let count = usize::try_from(out[EVENT_COUNT]).unwrap();
    (0..count)
        .map(|i| {
            let at = HEAD + i * EVENT_WIDTH;
            [out[at], out[at + 1], out[at + 2], out[at + 3], out[at + 4]]
        })
        .collect()
}

fn compare(case: Case) -> Seen {
    let seen = compare_with(obs_source_duplicate, case).unwrap_or_else(|| {
        panic!(
            "{case:?}: C {:?}, Rust {:?}",
            run(None, case),
            run(Some(obs_source_duplicate), case)
        )
    });
    // The source and a scene copy's source untouched, the new source only
    // in its copied fields, and no event lost.
    assert_eq!(
        (
            seen.0[SOURCE_UNCHANGED],
            seen.0[CREATED_UNCHANGED],
            seen.0[COPY_UNCHANGED],
            seen.0[OVERFLOW]
        ),
        (1, 1, 1, 0),
        "{case:?}"
    );
    seen
}

/// The copied fields of the fixture's source, or of a new source left as
/// built.
fn source_fields(noise: u8, bools: c_uint, source: bool) -> Vec<c_long> {
    let mut out = [0 as c_long; FIELDS];
    // SAFETY: the oracle writes FIELDS values.
    unsafe { oracle_sd_source_fields(noise, bools, source, out.as_mut_ptr()) };
    out.to_vec()
}

/// The calls of a scene-typed source on `case` and the source it returns.
fn expected_scene(case: Case, expected: &mut Vec<[c_long; EVENT_WIDTH]>) -> c_long {
    let answer = |bit| case.answers & bit != 0;
    let name = if case.named { NAME } else { NULL_ID };
    expected.push([EV_SCENE, SOURCE, 0, 0, 0]);
    let scene = if answer(A_SCENE) {
        Some(SCENE)
    } else {
        expected.push([EV_GROUP, SOURCE, 0, 0, 0]);
        answer(A_GROUP).then_some(GROUP)
    };
    if answer(A_SCENE) && !case.create_private {
        expected.push([EV_GET_REF, SOURCE, 0, 0, 0]);
        return if answer(A_REF) { SOURCE } else { NULL_ID };
    }
    let Some(scene) = scene else {
        return NULL_ID;
    };
    let dup_type = if case.create_private {
        DUP_PRIVATE_COPY
    } else {
        DUP_COPY
    };
    expected.push([EV_SCENE_DUPLICATE, scene, name, c_long::from(dup_type), 0]);
    if answer(A_SCENE_COPY) {
        expected.push([EV_SCENE_SOURCE, SCENE_COPY, 0, 0, 0]);
        COPY_SOURCE
    } else {
        expected.push([EV_SCENE_SOURCE, NULL_ID, 0, 0, 0]);
        NULL_ID
    }
}

/// The contract, from the original's observation of `case`.
fn check(case: Case, (out, message): &Seen) {
    let events = events(out);
    let fields = &out[FIELDS_AT..HEAD];
    let untouched = source_fields(case.noise, case.bools >> 2, false);
    let answer = |bit| case.answers & bit != 0;
    if !case.present {
        assert_eq!(message, "obs_source_duplicate: Null 'source' parameter");
        assert_eq!(events, [[EV_LOG, 400, 0, 0, 0]], "{case:?}");
        assert_eq!((out[LOGS], out[RESULT]), (1, NULL_ID));
        assert_eq!(fields, untouched);
        return;
    }
    assert_eq!(out[LOGS], 0, "{case:?}");
    let name = if case.named { NAME } else { NULL_ID };
    let private = c_long::from(case.create_private);
    let get_ref = [EV_GET_REF, SOURCE, 0, 0, 0];
    let ref_result = if answer(A_REF) { SOURCE } else { NULL_ID };
    let mut expected = Vec::new();
    let result;
    if case.source_type == TYPE_SCENE {
        result = expected_scene(case, &mut expected);
        assert_eq!(fields, untouched, "{case:?}");
    } else if case.output_flags & DO_NOT_DUPLICATE != 0 {
        expected.push(get_ref);
        result = ref_result;
        assert_eq!(fields, untouched, "{case:?}");
    } else {
        expected.push([EV_DATA_CREATE, 0, 0, 0, 0]);
        expected.push([EV_DATA_APPLY, SETTINGS, CONTEXT_SETTINGS, 0, 0]);
        if case.create_private {
            expected.push([EV_CREATE_PRIVATE, ID, name, SETTINGS, 0]);
        } else {
            expected.push([EV_CREATE, ID, name, SETTINGS, NULL_ID]);
        }
        if answer(A_CREATED) {
            expected.push([EV_SYNC_OFFSET, SOURCE, 0, 0, 0]);
            expected.push([EV_GET_VOLUME, SOURCE, 0, 0, 0]);
            expected.push([EV_DATA_APPLY, CREATED_PRIVATE, SOURCE_PRIVATE, 0, 0]);
            if case.source_type != TYPE_FILTER {
                expected.push([EV_DUPLICATE_FILTERS, CREATED, SOURCE, private, 0]);
            }
            assert_eq!(
                fields,
                source_fields(case.noise, case.bools & 3, true),
                "{case:?}"
            );
            result = CREATED;
        } else {
            assert_eq!(fields, untouched, "{case:?}");
            result = NULL_ID;
        }
        expected.push([EV_DATA_RELEASE, SETTINGS, 0, 0, 0]);
    }
    assert_eq!(events, expected, "{case:?}");
    assert_eq!(out[RESULT], result, "{case:?}");
}

/// Mutants of the export: the real core over a source that breaks one step.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Break {
    /// The volume copied from the user volume.
    VolumeFromUser,
    /// The filters copied for a filter too.
    FiltersForFilter,
    /// The settings not released after a failed create.
    KeepSettingsOnFailure,
    /// The group lookup skipped.
    NoGroup,
    /// The private flag dropped for the filter copies.
    PublicFilters,
    /// The muted flag not copied.
    NoMuted,
}

struct Mutant {
    source: NonNull<c_void>,
    new_name: *const c_char,
    broken: Break,
    created: Cell<bool>,
}

impl Source for Mutant {
    type Ref = NonNull<c_void>;
    type Scene = NonNull<c_void>;
    type Settings = *mut c_void;
    fn source_type(&self) -> i32 {
        // SAFETY: a live fixture source.
        unsafe { obs_source_duplicate_entry_type(self.source.as_ptr()) }
    }
    fn scene(&self) -> Option<NonNull<c_void>> {
        // SAFETY: as the export's.
        NonNull::new(unsafe { obs_source_duplicate_entry_scene(self.source.as_ptr()) })
    }
    fn group(&self) -> Option<NonNull<c_void>> {
        if self.broken == Break::NoGroup {
            return None;
        }
        // SAFETY: as the export's.
        NonNull::new(unsafe { obs_source_duplicate_entry_group(self.source.as_ptr()) })
    }
    fn get_ref(&self) -> Option<NonNull<c_void>> {
        // SAFETY: as the export's.
        NonNull::new(unsafe { obs_source_duplicate_entry_get_ref(self.source.as_ptr()) })
    }
    fn scene_duplicate(&self, scene: NonNull<c_void>, dup_type: i32) -> Option<NonNull<c_void>> {
        // SAFETY: as the export's.
        NonNull::new(unsafe {
            obs_source_duplicate_entry_scene_duplicate(scene.as_ptr(), self.new_name, dup_type)
        })
    }
    fn scene_source(&self, scene: Option<NonNull<c_void>>) -> Option<NonNull<c_void>> {
        let scene = scene.map_or(ptr::null_mut(), NonNull::as_ptr);
        // SAFETY: as the export's.
        NonNull::new(unsafe { obs_source_duplicate_entry_scene_source(scene) })
    }
    fn output_flags(&self) -> u32 {
        // SAFETY: as the export's.
        unsafe { obs_source_duplicate_entry_output_flags(self.source.as_ptr()) }
    }
    fn settings_create(&self) -> *mut c_void {
        // SAFETY: as the export's.
        unsafe { obs_source_duplicate_entry_settings_create() }
    }
    fn settings_apply(&self, settings: *mut c_void) {
        // SAFETY: as the export's.
        unsafe { obs_source_duplicate_entry_settings_apply(settings, self.source.as_ptr()) }
    }
    fn create(&self, settings: *mut c_void, private: bool) -> Option<NonNull<c_void>> {
        // SAFETY: as the export's.
        let created = NonNull::new(unsafe {
            obs_source_duplicate_entry_create(
                self.source.as_ptr(),
                self.new_name,
                settings,
                private,
            )
        });
        self.created.set(created.is_some());
        created
    }
    fn settings_release(&self, settings: *mut c_void) {
        if self.broken == Break::KeepSettingsOnFailure && !self.created.get() {
            return;
        }
        // SAFETY: as the export's.
        unsafe { obs_source_duplicate_entry_settings_release(settings) }
    }
    fn audio_mixers(&self) -> u32 {
        // SAFETY: as the export's.
        unsafe { obs_source_duplicate_entry_audio_mixers(self.source.as_ptr()) }
    }
    fn set_audio_mixers(&self, s: NonNull<c_void>, value: u32) {
        // SAFETY: as the export's.
        unsafe { obs_source_duplicate_entry_set_audio_mixers(s.as_ptr(), value) }
    }
    fn sync_offset(&self) -> i64 {
        // SAFETY: as the export's.
        unsafe { obs_source_duplicate_entry_sync_offset(self.source.as_ptr()) }
    }
    fn set_sync_offset(&self, s: NonNull<c_void>, value: i64) {
        // SAFETY: as the export's.
        unsafe { obs_source_duplicate_entry_set_sync_offset(s.as_ptr(), value) }
    }
    fn user_volume(&self) -> f32 {
        // SAFETY: as the export's.
        unsafe { obs_source_duplicate_entry_user_volume(self.source.as_ptr()) }
    }
    fn set_user_volume(&self, s: NonNull<c_void>, value: f32) {
        // SAFETY: as the export's.
        unsafe { obs_source_duplicate_entry_set_user_volume(s.as_ptr(), value) }
    }
    fn user_muted(&self) -> bool {
        // SAFETY: as the export's.
        unsafe { obs_source_duplicate_entry_user_muted(self.source.as_ptr()) }
    }
    fn set_user_muted(&self, s: NonNull<c_void>, value: bool) {
        // SAFETY: as the export's.
        unsafe { obs_source_duplicate_entry_set_user_muted(s.as_ptr(), value) }
    }
    fn volume(&self) -> f32 {
        if self.broken == Break::VolumeFromUser {
            // SAFETY: as the export's.
            return unsafe { obs_source_duplicate_entry_user_volume(self.source.as_ptr()) };
        }
        // SAFETY: as the export's.
        unsafe { obs_source_duplicate_entry_volume(self.source.as_ptr()) }
    }
    fn set_volume(&self, s: NonNull<c_void>, value: f32) {
        // SAFETY: as the export's.
        unsafe { obs_source_duplicate_entry_set_volume(s.as_ptr(), value) }
    }
    fn muted(&self) -> bool {
        // SAFETY: as the export's.
        unsafe { obs_source_duplicate_entry_muted(self.source.as_ptr()) }
    }
    fn set_muted(&self, s: NonNull<c_void>, value: bool) {
        if self.broken != Break::NoMuted {
            // SAFETY: as the export's.
            unsafe { obs_source_duplicate_entry_set_muted(s.as_ptr(), value) }
        }
    }
    fn flags(&self) -> u32 {
        // SAFETY: as the export's.
        unsafe { obs_source_duplicate_entry_flags(self.source.as_ptr()) }
    }
    fn set_flags(&self, s: NonNull<c_void>, value: u32) {
        // SAFETY: as the export's.
        unsafe { obs_source_duplicate_entry_set_flags(s.as_ptr(), value) }
    }
    fn private_settings_apply(&self, s: NonNull<c_void>) {
        // SAFETY: as the export's.
        unsafe {
            obs_source_duplicate_entry_private_settings_apply(s.as_ptr(), self.source.as_ptr());
        }
        if self.broken == Break::FiltersForFilter && self.source_type() == TYPE_FILTER {
            // SAFETY: as the export's.
            unsafe { obs_source_duplicate_entry_filters(s.as_ptr(), self.source.as_ptr(), true) }
        }
    }
    fn duplicate_filters(&self, s: NonNull<c_void>, private: bool) {
        let private = private && self.broken != Break::PublicFilters;
        // SAFETY: as the export's.
        unsafe { obs_source_duplicate_entry_filters(s.as_ptr(), self.source.as_ptr(), private) }
    }
}

fn broken(b: u8) -> Break {
    match b {
        0 => Break::VolumeFromUser,
        1 => Break::FiltersForFilter,
        2 => Break::KeepSettingsOnFailure,
        3 => Break::NoGroup,
        4 => Break::PublicFilters,
        _ => Break::NoMuted,
    }
}

unsafe extern "C" fn mutant_duplicate<const B: u8>(
    source: *mut c_void,
    new_name: *const c_char,
    create_private: bool,
) -> *mut c_void {
    let Some(source) = NonNull::new(source) else {
        // SAFETY: as the export's.
        return unsafe { obs_source_duplicate(ptr::null_mut(), new_name, create_private) };
    };
    let mutant = Mutant {
        source,
        new_name,
        broken: broken(B),
        created: Cell::new(false),
    };
    duplicate(Some(&mutant), create_private, |_, _| panic!("logged"))
        .map_or(ptr::null_mut(), NonNull::as_ptr)
}

/// Every type, both flag patterns, every answer, name, privacy and bools
/// pattern at one noise byte, through `f`.
fn grid(noise: u8, mut f: impl FnMut(Case)) {
    for source_type in TYPES {
        for output_flags in [0, DO_NOT_DUPLICATE, !DO_NOT_DUPLICATE, u32::MAX] {
            for answers in 0..ANSWERS {
                for named in [false, true] {
                    for create_private in [false, true] {
                        for bools in [0b0000, 0b0011, 0b1100, 0b1001, 0b0110] {
                            f(Case {
                                present: true,
                                noise,
                                source_type,
                                output_flags,
                                answers,
                                named,
                                create_private,
                                bools,
                            });
                        }
                    }
                }
            }
        }
    }
}

/// Each mutant disagrees with the original C somewhere on the grid.
fn rejects_mutants() {
    let mutants: [Duplicate; 6] = [
        mutant_duplicate::<0>,
        mutant_duplicate::<1>,
        mutant_duplicate::<2>,
        mutant_duplicate::<3>,
        mutant_duplicate::<4>,
        mutant_duplicate::<5>,
    ];
    for (i, mutant) in mutants.into_iter().enumerate() {
        let mut caught = false;
        grid(0x5a, |case| {
            if !caught {
                caught = compare_with(mutant, case).is_none();
            }
        });
        assert!(
            caught,
            "obs_source_duplicate mutant {i} agreed with the original C"
        );
    }
}

/// NULL, then the grid, at several noise bytes.
fn every_branch() {
    for noise in [0x00, 0x5a, 0xa5, 0xff] {
        for create_private in [false, true] {
            let case = Case {
                present: false,
                noise,
                source_type: 0,
                output_flags: 0,
                answers: 0,
                named: true,
                create_private,
                bools: 0,
            };
            check(case, &compare(case));
        }
        grid(noise, |case| check(case, &compare(case)));
    }
}

#[test]
fn source_duplicate_entry_matches_c() {
    if !crate::output::isolated(concat!(
        module_path!(),
        "::source_duplicate_entry_matches_c"
    )) {
        return;
    }
    // SAFETY: plain reads of the oracle's constants.
    assert_eq!(
        unsafe {
            [
                oracle_sd_layout(0),
                oracle_sd_layout(1),
                oracle_sd_layout(2),
            ]
        },
        [OUT, EVENT_WIDTH, FIELDS]
    );
    every_branch();
    rejects_mutants();
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(
                any::<(bool, u8, i32, u32)>(),
                0..ANSWERS,
                any::<(bool, bool)>(),
                0..16u32,
            ),
            |(
                (present, noise, source_type, output_flags),
                answers,
                (named, create_private),
                bools,
            )| {
                let case = Case {
                    present,
                    noise,
                    source_type,
                    output_flags,
                    answers,
                    named,
                    create_private,
                    bools,
                };
                check(case, &compare(case));
                Ok(())
            },
        )
        .unwrap();
}
