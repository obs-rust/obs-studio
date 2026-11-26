//! Tier 3: the original C async-frame entry points (the extracted
//! `obs-source-frame-wrappers.c.h`) against the Rust exports, each on a
//! real private source (`source_frame_wrappers.c`). Both run over the same
//! production adapters with `destroying()` and the three workers recorded;
//! for every call the recorded calls must match: which worker, the source,
//! whether the frame pointer is NULL, the caller's or a copy, and the copy's
//! every field as bits. The caller's frame must be unchanged after both.
//! No intentional differences or exclusions.
use core::ffi::{c_int, c_long, c_void};
use obs_source::ffi::source_frame_wrappers as rust;
use obs_source::frame_wrappers::{Frame, Frame2, MAX_AV_PLANES};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_obs_source_output_video(source: *mut c_void, frame: *const Frame);
    fn oracle_obs_source_output_video2(source: *mut c_void, frame: *const Frame2);
    fn oracle_obs_source_preload_video(source: *mut c_void, frame: *const Frame);
    fn oracle_obs_source_preload_video2(source: *mut c_void, frame: *const Frame2);
    fn oracle_obs_source_set_video_frame(source: *mut c_void, frame: *const Frame);
    fn oracle_obs_source_set_video_frame2(source: *mut c_void, frame: *const Frame2);
    fn oracle_fw_events(
        kinds: *mut c_int,
        sources: *mut *const c_void,
        frames: *mut *const c_void,
        copies: *mut Frame,
        max: usize,
    ) -> usize;
    fn oracle_fw_source_new(destroying: c_long) -> *mut c_void;
    fn oracle_fw_source_free(source: *mut c_void);
    fn oracle_fw_frame_layout(out: *mut usize, max: usize) -> usize;
    fn obs_source_frame_wrappers_preload(source: *mut c_void, frame: *const Frame);
    fn obs_source_frame_wrappers_set(source: *mut c_void, frame: *const Frame);
    fn obs_source_frame_wrappers_output(source: *mut c_void, frame: *const Frame);
}

type Entry = unsafe extern "C" fn(*mut c_void, *const Frame);
type Entry2 = unsafe extern "C" fn(*mut c_void, *const Frame2);

/// The six entry points of one side, in a fixed order.
#[derive(Clone, Copy)]
struct Side {
    frame: [Entry; 3],
    frame2: [Entry2; 3],
}

const C: Side = Side {
    frame: [
        oracle_obs_source_output_video,
        oracle_obs_source_preload_video,
        oracle_obs_source_set_video_frame,
    ],
    frame2: [
        oracle_obs_source_output_video2,
        oracle_obs_source_preload_video2,
        oracle_obs_source_set_video_frame2,
    ],
};

const RUST: Side = Side {
    frame: [
        rust::obs_source_output_video,
        rust::obs_source_preload_video,
        rust::obs_source_set_video_frame,
    ],
    frame2: [
        rust::obs_source_output_video2,
        rust::obs_source_preload_video2,
        rust::obs_source_set_video_frame2,
    ],
};

const DESTROYING: c_int = 1;
const OUTPUT: c_int = 2;
const PRELOAD: c_int = 3;
const SET: c_int = 4;

#[derive(Debug, Clone, Copy)]
enum Input {
    Null,
    Frame(Frame),
    Frame2(Frame2),
}

#[derive(Debug, Clone, Copy)]
struct Call {
    /// 0 output, 1 preload, 2 set.
    entry: usize,
    destroying: c_long,
    input: Input,
}

/// A frame's fields as bits, so floats (NaN included) compare exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Bits {
    data: [usize; MAX_AV_PLANES],
    linesize: [u32; MAX_AV_PLANES],
    width: u32,
    height: u32,
    timestamp: u64,
    format: c_int,
    color_matrix: [u32; 16],
    full_range: bool,
    max_luminance: u16,
    color_range_min: [u32; 3],
    color_range_max: [u32; 3],
    flip: bool,
    flags: u8,
    trc: u8,
    refs: c_long,
    prev_frame: bool,
}

fn bits(f: &Frame) -> Bits {
    Bits {
        data: f.data.map(|p| p as usize),
        linesize: f.linesize,
        width: f.width,
        height: f.height,
        timestamp: f.timestamp,
        format: f.format,
        color_matrix: f.color_matrix.map(f32::to_bits),
        full_range: f.full_range,
        max_luminance: f.max_luminance,
        color_range_min: f.color_range_min.map(f32::to_bits),
        color_range_max: f.color_range_max.map(f32::to_bits),
        flip: f.flip,
        flags: f.flags,
        trc: f.trc,
        refs: f.refs,
        prev_frame: f.prev_frame,
    }
}

fn bits2(f: &Frame2) -> (Bits, c_int) {
    (
        Bits {
            data: f.data.map(|p| p as usize),
            linesize: f.linesize,
            width: f.width,
            height: f.height,
            timestamp: f.timestamp,
            format: f.format,
            color_matrix: f.color_matrix.map(f32::to_bits),
            full_range: false,
            max_luminance: 0,
            color_range_min: f.color_range_min.map(f32::to_bits),
            color_range_max: f.color_range_max.map(f32::to_bits),
            flip: f.flip,
            flags: f.flags,
            trc: f.trc,
            refs: 0,
            prev_frame: false,
        },
        f.range,
    )
}

/// The frame pointer a worker got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ptr {
    Null,
    Caller,
    Copy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Event {
    kind: c_int,
    own_source: bool,
    frame: Ptr,
    copy: Option<Bits>,
}

/// Runs one call on one side; returns the recorded calls.
fn run(side: Side, call: &Call) -> Vec<Event> {
    // SAFETY: a fresh bzalloc'd source the recorders only read; the frame
    // is NULL or a local the export only reads; the event buffers hold the
    // max passed.
    unsafe {
        let source = oracle_fw_source_new(call.destroying);
        assert!(!source.is_null());
        let mut frame = None;
        let mut frame2 = None;
        if let Input::Frame(f) = call.input {
            frame = Some(f);
        }
        if let Input::Frame2(f) = call.input {
            frame2 = Some(f);
        }
        let caller: *const c_void = match (&frame, &frame2) {
            (Some(f), _) => core::ptr::from_ref(f).cast(),
            (_, Some(f)) => core::ptr::from_ref(f).cast(),
            _ => core::ptr::null(),
        };
        oracle_fw_events(
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            0,
        );
        match call.input {
            Input::Null => (side.frame[call.entry])(source, core::ptr::null()),
            Input::Frame(_) => (side.frame[call.entry])(source, caller.cast()),
            Input::Frame2(_) => (side.frame2[call.entry])(source, caller.cast()),
        }
        // The caller's frame is only read.
        match (call.input, &frame, &frame2) {
            (Input::Frame(before), Some(after), _) => assert_eq!(bits(&before), bits(after)),
            (Input::Frame2(before), _, Some(after)) => assert_eq!(bits2(&before), bits2(after)),
            _ => {}
        }
        let mut kinds = [0; 8];
        let mut sources = [core::ptr::null(); 8];
        let mut frames = [core::ptr::null(); 8];
        let mut copies: [Frame; 8] = core::mem::zeroed();
        let n = oracle_fw_events(
            kinds.as_mut_ptr(),
            sources.as_mut_ptr(),
            frames.as_mut_ptr(),
            copies.as_mut_ptr(),
            kinds.len(),
        );
        assert!(n <= kinds.len());
        let events = (0..n)
            .map(|i| Event {
                kind: kinds[i],
                own_source: sources[i] == source.cast_const(),
                frame: if frames[i].is_null() {
                    Ptr::Null
                } else if frames[i] == caller {
                    Ptr::Caller
                } else {
                    Ptr::Copy
                },
                copy: (!frames[i].is_null()).then(|| bits(&copies[i])),
            })
            .collect();
        oracle_fw_source_free(source);
        events
    }
}

fn check(call: &Call) -> Vec<Event> {
    let c = run(C, call);
    let r = run(RUST, call);
    assert_eq!(c, r, "{call:?}");
    c
}

fn agree(side: Side, calls: &[Call]) -> bool {
    calls.iter().all(|call| run(C, call) == run(side, call))
}

/// A plane address; only compared, never read.
fn plane(i: usize) -> *mut u8 {
    core::ptr::dangling_mut::<u8>().wrapping_add(i)
}

fn frame(format: c_int, full_range: bool) -> Frame {
    let mut data = [core::ptr::null_mut(); MAX_AV_PLANES];
    data[0] = plane(0x10);
    data[2] = plane(0x20);
    data[7] = plane(0x70);
    Frame {
        data,
        linesize: [8, 0, 4, 0, 0, 0, 0, 9],
        width: 640,
        height: 360,
        timestamp: 0x1234_5678_9abc,
        format,
        color_matrix: core::array::from_fn(|i| f32::from(u8::try_from(i).unwrap()) + 0.5),
        full_range,
        max_luminance: 1000,
        color_range_min: [0.0625, -0.0, f32::NAN],
        color_range_max: [0.75, f32::INFINITY, 1e-40],
        flip: true,
        flags: 0xa5,
        trc: 3,
        refs: 77,
        prev_frame: true,
    }
}

fn frame2(format: c_int, range: c_int) -> Frame2 {
    let f = frame(format, false);
    Frame2 {
        data: f.data,
        linesize: f.linesize,
        width: f.width,
        height: f.height,
        timestamp: f.timestamp,
        format,
        range,
        color_matrix: f.color_matrix,
        color_range_min: f.color_range_min,
        color_range_max: f.color_range_max,
        flip: f.flip,
        flags: f.flags,
        trc: f.trc,
    }
}

const BGRA: c_int = 7;
const I420: c_int = 1;
const PARTIAL: c_int = 1;
const FULL: c_int = 2;

/// Every contract point of `test_source_frame_wrappers` and the issue, one
/// call each.
fn characterized() -> Vec<Call> {
    let mut calls = Vec::new();
    for entry in 0..3 {
        for destroying in [1, 0] {
            calls.push(Call {
                entry,
                destroying,
                input: Input::Null,
            });
            calls.push(Call {
                entry,
                destroying,
                input: Input::Frame(frame(BGRA, false)),
            });
            calls.push(Call {
                entry,
                destroying,
                input: Input::Frame(frame(I420, false)),
            });
            calls.push(Call {
                entry,
                destroying,
                input: Input::Frame2(frame2(BGRA, PARTIAL)),
            });
            calls.push(Call {
                entry,
                destroying,
                input: Input::Frame2(frame2(I420, FULL)),
            });
        }
    }
    calls
}

#[test]
fn frame_wrappers_layout_matches_c() {
    let mut c = [0usize; 64];
    // SAFETY: the buffer holds the max passed.
    let n = unsafe { oracle_fw_frame_layout(c.as_mut_ptr(), c.len()) };
    let rust = [
        size_of::<Frame>(),
        core::mem::offset_of!(Frame, data),
        core::mem::offset_of!(Frame, linesize),
        core::mem::offset_of!(Frame, width),
        core::mem::offset_of!(Frame, height),
        core::mem::offset_of!(Frame, timestamp),
        core::mem::offset_of!(Frame, format),
        core::mem::offset_of!(Frame, color_matrix),
        core::mem::offset_of!(Frame, full_range),
        core::mem::offset_of!(Frame, max_luminance),
        core::mem::offset_of!(Frame, color_range_min),
        core::mem::offset_of!(Frame, color_range_max),
        core::mem::offset_of!(Frame, flip),
        core::mem::offset_of!(Frame, flags),
        core::mem::offset_of!(Frame, trc),
        core::mem::offset_of!(Frame, refs),
        core::mem::offset_of!(Frame, prev_frame),
        size_of::<Frame2>(),
        core::mem::offset_of!(Frame2, data),
        core::mem::offset_of!(Frame2, linesize),
        core::mem::offset_of!(Frame2, width),
        core::mem::offset_of!(Frame2, height),
        core::mem::offset_of!(Frame2, timestamp),
        core::mem::offset_of!(Frame2, format),
        core::mem::offset_of!(Frame2, range),
        core::mem::offset_of!(Frame2, color_matrix),
        core::mem::offset_of!(Frame2, color_range_min),
        core::mem::offset_of!(Frame2, color_range_max),
        core::mem::offset_of!(Frame2, flip),
        core::mem::offset_of!(Frame2, flags),
        core::mem::offset_of!(Frame2, trc),
    ];
    assert_eq!(&c[..n], &rust[..]);
}

#[test]
fn frame_wrappers_match_c() {
    if !crate::output::isolated(concat!(module_path!(), "::frame_wrappers_match_c")) {
        return;
    }
    let calls = characterized();
    let events: Vec<Vec<Event>> = calls.iter().map(check).collect();
    pin(&calls, &events);
}

/// The shared answers, pinned so a change on both sides still fails.
fn pin(calls: &[Call], events: &[Vec<Event>]) {
    let worker = [OUTPUT, PRELOAD, SET];
    for (call, events) in calls.iter().zip(events) {
        // destroying() first, on the caller's source, with no frame.
        assert_eq!(
            events[0],
            Event {
                kind: DESTROYING,
                own_source: true,
                frame: Ptr::Null,
                copy: None
            }
        );
        if call.destroying != 0 {
            assert_eq!(events.len(), 1, "{call:?}");
            continue;
        }
        assert_eq!(events.len(), 2, "{call:?}");
        let got = &events[1];
        assert!(got.own_source);
        match call.input {
            Input::Null => {
                // set_video_frame(NULL) reaches the preload worker.
                let kind = if call.entry == 0 { OUTPUT } else { PRELOAD };
                assert_eq!((got.kind, got.frame), (kind, Ptr::Null), "{call:?}");
            }
            Input::Frame(f) => {
                assert_eq!((got.kind, got.frame), (worker[call.entry], Ptr::Copy));
                // Whole copy; BGRA becomes full range, YUV keeps it.
                let mut want = bits(&f);
                want.full_range = f.format == BGRA || f.full_range;
                assert_eq!(got.copy.as_ref(), Some(&want), "{call:?}");
            }
            Input::Frame2(f) => {
                assert_eq!((got.kind, got.frame), (worker[call.entry], Ptr::Copy));
                let (mut want, range) = bits2(&f);
                want.full_range = range == FULL;
                assert_eq!(got.copy.as_ref(), Some(&want), "{call:?}");
            }
        }
    }
}

/// Mutants of the exports: each must disagree with the original.
unsafe extern "C" fn set_null_to_set(source: *mut c_void, frame: *const Frame) {
    if frame.is_null() {
        // SAFETY: the oracle source; the quirk "fixed": the set worker.
        unsafe {
            if !rust_destroying(source) {
                obs_source_frame_wrappers_set(source, frame);
            }
        }
    } else {
        // SAFETY: as the export.
        unsafe { rust::obs_source_set_video_frame(source, frame) };
    }
}
unsafe extern "C" fn output_passes_caller(source: *mut c_void, frame: *const Frame) {
    // SAFETY: the oracle source, the caller's frame unchanged, no destroying
    // check of its own: the worker gets the caller's pointer.
    unsafe {
        if rust_destroying(source) {
            return;
        }
        obs_source_frame_wrappers_output(source, frame);
    }
}
unsafe extern "C" fn preload_null_before_destroying(source: *mut c_void, frame: *const Frame) {
    if frame.is_null() {
        // SAFETY: the oracle source.
        unsafe { obs_source_frame_wrappers_preload(source, frame) };
    } else {
        // SAFETY: as the export.
        unsafe { rust::obs_source_preload_video(source, frame) };
    }
}
unsafe extern "C" fn output2_keeps_range_rule_of_frame(source: *mut c_void, frame: *const Frame2) {
    // SAFETY: NULL or the caller's frame, only read; the oracle source.
    unsafe {
        match frame.as_ref() {
            None => rust::obs_source_output_video2(source, frame),
            Some(f) => {
                // The obs_source_frame rule instead of the range's.
                let mut g = *f;
                g.range = if obs_source::frame_copy::full_range(f.format, false) {
                    FULL
                } else {
                    PARTIAL
                };
                rust::obs_source_output_video2(source, &raw const g);
            }
        }
    }
}

/// `destroying()` through the adapter, as the export calls it.
unsafe fn rust_destroying(source: *mut c_void) -> bool {
    unsafe extern "C" {
        fn obs_source_frame_wrappers_destroying(source: *mut c_void) -> bool;
    }
    // SAFETY: the oracle source.
    unsafe { obs_source_frame_wrappers_destroying(source) }
}

#[test]
fn frame_wrappers_mutants_are_caught() {
    if !crate::output::isolated(concat!(
        module_path!(),
        "::frame_wrappers_mutants_are_caught"
    )) {
        return;
    }
    let calls = characterized();
    assert!(agree(RUST, &calls));
    let mutants = [
        Side {
            frame: [RUST.frame[0], RUST.frame[1], set_null_to_set as Entry],
            ..RUST
        },
        Side {
            frame: [output_passes_caller as Entry, RUST.frame[1], RUST.frame[2]],
            ..RUST
        },
        Side {
            frame: [
                RUST.frame[0],
                preload_null_before_destroying as Entry,
                RUST.frame[2],
            ],
            ..RUST
        },
        Side {
            frame2: [
                output2_keeps_range_rule_of_frame as Entry2,
                RUST.frame2[1],
                RUST.frame2[2],
            ],
            ..RUST
        },
    ];
    for (i, mutant) in mutants.into_iter().enumerate() {
        assert!(!agree(mutant, &calls), "mutant {i} agreed with C");
    }
}

fn floats<const N: usize>() -> impl Strategy<Value = [f32; N]> {
    proptest::array::uniform(any::<u32>().prop_map(f32::from_bits))
}

fn planes() -> impl Strategy<Value = [*mut u8; MAX_AV_PLANES]> {
    proptest::array::uniform(prop_oneof![Just(0usize), 1usize..0x1_0000].prop_map(|a| {
        if a == 0 {
            core::ptr::null_mut()
        } else {
            plane(a)
        }
    }))
}

fn any_frame() -> impl Strategy<Value = Frame> {
    (
        planes(),
        any::<[u32; MAX_AV_PLANES]>(),
        (any::<u32>(), any::<u32>(), any::<u64>(), -3 as c_int..40),
        floats::<16>(),
        (any::<bool>(), any::<u16>()),
        (floats::<3>(), floats::<3>()),
        (
            any::<bool>(),
            any::<u8>(),
            any::<u8>(),
            any::<c_long>(),
            any::<bool>(),
        ),
    )
        .prop_map(
            |(
                data,
                linesize,
                (width, height, timestamp, format),
                color_matrix,
                fr,
                ranges,
                tail,
            )| {
                Frame {
                    data,
                    linesize,
                    width,
                    height,
                    timestamp,
                    format,
                    color_matrix,
                    full_range: fr.0,
                    max_luminance: fr.1,
                    color_range_min: ranges.0,
                    color_range_max: ranges.1,
                    flip: tail.0,
                    flags: tail.1,
                    trc: tail.2,
                    refs: tail.3,
                    prev_frame: tail.4,
                }
            },
        )
}

fn any_frame2() -> impl Strategy<Value = Frame2> {
    (any_frame(), -3 as c_int..6).prop_map(|(f, range)| Frame2 {
        data: f.data,
        linesize: f.linesize,
        width: f.width,
        height: f.height,
        timestamp: f.timestamp,
        format: f.format,
        range,
        color_matrix: f.color_matrix,
        color_range_min: f.color_range_min,
        color_range_max: f.color_range_max,
        flip: f.flip,
        flags: f.flags,
        trc: f.trc,
    })
}

fn any_call() -> impl Strategy<Value = Call> {
    (
        0usize..3,
        prop_oneof![Just(0 as c_long), Just(1), any::<c_long>()],
        prop_oneof![
            Just(Input::Null),
            any_frame().prop_map(Input::Frame),
            any_frame2().prop_map(Input::Frame2),
        ],
    )
        .prop_map(|(entry, destroying, input)| Call {
            entry,
            destroying,
            input,
        })
}

#[test]
fn frame_wrappers_random_frames_match_c() {
    if !crate::output::isolated(concat!(
        module_path!(),
        "::frame_wrappers_random_frames_match_c"
    )) {
        return;
    }
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(512),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(&any_call(), |call| {
            check(&call);
            Ok(())
        })
        .unwrap();
}
