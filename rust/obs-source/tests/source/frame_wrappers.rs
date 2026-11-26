//! Tier 1: `obs_source::frame_wrappers` over a recording model of the three
//! C workers, mirroring every `test_source_frame_wrappers` case plus what
//! the cmocka test cannot see: which worker each entry calls (the
//! `set_video_frame` NULL quirk), the order of the calls and the exact frame
//! each worker receives, field by field.
use obs_source::frame_copy::{video_format, video_range};
use obs_source::frame_wrappers::{
    Frame, Frame2, Host, MAX_AV_PLANES, from_frame, from_frame2, output_video, output_video2,
    preload_video, preload_video2, set_video_frame, set_video_frame2,
};
use std::cell::RefCell;

/// A frame's fields as bits, so floats compare exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Bits {
    data: [usize; MAX_AV_PLANES],
    linesize: [u32; MAX_AV_PLANES],
    width: u32,
    height: u32,
    timestamp: u64,
    format: i32,
    color_matrix: [u32; 16],
    full_range: bool,
    max_luminance: u16,
    color_range_min: [u32; 3],
    color_range_max: [u32; 3],
    flip: bool,
    flags: u8,
    trc: u8,
    refs: core::ffi::c_long,
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    Destroying,
    Output(Option<Bits>),
    Preload(Option<Bits>),
    Set(Option<Bits>),
}

struct Model {
    destroying: bool,
    events: RefCell<Vec<Event>>,
    /// The address of each frame a worker got, to check it is not the
    /// caller's.
    seen: RefCell<Vec<usize>>,
}

impl Model {
    fn new(destroying: bool) -> Self {
        Model {
            destroying,
            events: RefCell::new(Vec::new()),
            seen: RefCell::new(Vec::new()),
        }
    }
    fn events(&self) -> Vec<Event> {
        std::mem::take(&mut self.events.borrow_mut())
    }
    fn record(&self, frame: Option<&Frame>) -> Option<Bits> {
        frame.map(|f| {
            self.seen.borrow_mut().push(core::ptr::from_ref(f) as usize);
            bits(f)
        })
    }
}

impl Host for Model {
    type Source = ();
    fn destroying(&self, (): ()) -> bool {
        self.events.borrow_mut().push(Event::Destroying);
        self.destroying
    }
    fn output(&self, (): (), frame: Option<&Frame>) {
        let b = self.record(frame);
        self.events.borrow_mut().push(Event::Output(b));
    }
    fn preload(&self, (): (), frame: Option<&Frame>) {
        let b = self.record(frame);
        self.events.borrow_mut().push(Event::Preload(b));
    }
    fn set(&self, (): (), frame: Option<&Frame>) {
        let b = self.record(frame);
        self.events.borrow_mut().push(Event::Set(b));
    }
}

/// A plane address; only compared, never read.
fn plane(i: usize) -> *mut u8 {
    core::ptr::dangling_mut::<u8>().wrapping_add(i)
}

fn frame(format: i32, full_range: bool) -> Frame {
    let mut data = [core::ptr::null_mut(); MAX_AV_PLANES];
    data[0] = plane(0);
    data[1] = plane(1);
    data[7] = plane(3);
    Frame {
        data,
        linesize: [8, 4, 4, 0, 0, 0, 0, 9],
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

fn frame2(format: i32, range: i32) -> Frame2 {
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

/// The frame an `obs_source_frame2` must become: every field it has, the
/// rest zero, `max_luminance` 0.
fn built(f: &Frame2, full_range: bool) -> Bits {
    Bits {
        data: f.data.map(|p| p as usize),
        linesize: f.linesize,
        width: f.width,
        height: f.height,
        timestamp: f.timestamp,
        format: f.format,
        color_matrix: f.color_matrix.map(f32::to_bits),
        full_range,
        max_luminance: 0,
        color_range_min: f.color_range_min.map(f32::to_bits),
        color_range_max: f.color_range_max.map(f32::to_bits),
        flip: f.flip,
        flags: f.flags,
        trc: f.trc,
        refs: 0,
        prev_frame: false,
    }
}

/// The caller's frame with only `full_range` replaced.
fn copied(f: &Frame, full_range: bool) -> Bits {
    Bits {
        full_range,
        ..bits(f)
    }
}

use Event::*;

type Entry = fn(&Model, (), Option<&Frame>);
type Entry2 = fn(&Model, (), Option<&Frame2>);

const ENTRIES: [Entry; 3] = [output_video, preload_video, set_video_frame];
const ENTRIES2: [Entry2; 3] = [output_video2, preload_video2, set_video_frame2];

#[test]
fn destroying_returns_first() {
    let f = frame(video_format::BGRA, false);
    let f2 = frame2(video_format::BGRA, video_range::FULL);
    for entry in ENTRIES {
        let m = Model::new(true);
        entry(&m, (), Some(&f));
        entry(&m, (), None);
        assert_eq!(m.events(), [Destroying, Destroying]);
    }
    for entry in ENTRIES2 {
        let m = Model::new(true);
        entry(&m, (), Some(&f2));
        entry(&m, (), None);
        assert_eq!(m.events(), [Destroying, Destroying]);
    }
}

#[test]
fn null_frame_goes_to_the_worker() {
    let m = Model::new(false);
    output_video(&m, (), None);
    output_video2(&m, (), None);
    preload_video(&m, (), None);
    preload_video2(&m, (), None);
    assert_eq!(
        m.events(),
        [
            Destroying,
            Output(None),
            Destroying,
            Output(None),
            Destroying,
            Preload(None),
            Destroying,
            Preload(None)
        ]
    );
}

#[test]
fn set_video_frame_null_calls_the_preload_worker() {
    // Upstream quirk, characterized, not endorsed.
    let m = Model::new(false);
    set_video_frame(&m, (), None);
    set_video_frame2(&m, (), None);
    assert_eq!(
        m.events(),
        [Destroying, Preload(None), Destroying, Preload(None)]
    );
}

#[test]
fn frame_entries_copy_with_full_range_replaced() {
    for (format, given, expected) in [
        (video_format::BGRA, false, true),
        (video_format::BGRA, true, true),
        (video_format::I420, false, false),
        (video_format::I420, true, true),
        (video_format::NV12, false, false),
        // Outside the enum: not YUV, so full range.
        (-1, false, true),
        (1000, false, true),
    ] {
        let f = frame(format, given);
        let want = copied(&f, expected);
        let m = Model::new(false);
        output_video(&m, (), Some(&f));
        preload_video(&m, (), Some(&f));
        set_video_frame(&m, (), Some(&f));
        assert_eq!(
            m.events(),
            [
                Destroying,
                Output(Some(want.clone())),
                Destroying,
                Preload(Some(want.clone())),
                Destroying,
                Set(Some(want.clone()))
            ],
            "format {format} full_range {given}"
        );
        let caller = core::ptr::from_ref(&f) as usize;
        assert!(m.seen.borrow().iter().all(|&p| p != caller));
    }
}

#[test]
fn frame2_entries_build_a_frame() {
    for (format, range, expected) in [
        (video_format::BGRA, video_range::DEFAULT, true),
        (video_format::BGRA, video_range::PARTIAL, false),
        (video_format::BGRA, video_range::FULL, true),
        (video_format::I420, video_range::DEFAULT, false),
        (video_format::I420, video_range::PARTIAL, false),
        (video_format::I420, video_range::FULL, true),
        // Outside the enums.
        (video_format::I420, 7, false),
        (-1, video_range::DEFAULT, true),
    ] {
        let f = frame2(format, range);
        let want = built(&f, expected);
        let m = Model::new(false);
        output_video2(&m, (), Some(&f));
        preload_video2(&m, (), Some(&f));
        set_video_frame2(&m, (), Some(&f));
        assert_eq!(
            m.events(),
            [
                Destroying,
                Output(Some(want.clone())),
                Destroying,
                Preload(Some(want.clone())),
                Destroying,
                Set(Some(want.clone()))
            ],
            "format {format} range {range}"
        );
    }
}

#[test]
fn from_frame_keeps_every_other_field() {
    let f = frame(video_format::RGBA, false);
    assert_eq!(bits(&from_frame(&f)), copied(&f, true));
    let mut odd = frame(video_format::I444, true);
    odd.refs = -5;
    odd.prev_frame = false;
    odd.max_luminance = u16::MAX;
    assert_eq!(bits(&from_frame(&odd)), copied(&odd, true));
}

#[test]
fn from_frame2_zeroes_what_frame2_lacks() {
    let f = frame2(video_format::P010, video_range::FULL);
    let b = bits(&from_frame2(&f));
    assert_eq!(b, built(&f, true));
    assert_eq!((b.max_luminance, b.refs, b.prev_frame), (0, 0, false));
}
