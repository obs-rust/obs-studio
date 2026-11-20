//! Tier 1 mirrors every assertion of the unchanged
//! test/cmocka/test_output_get_last_error.c (#5294) on a recording model of
//! the output's fields. Errors are allocation ids, so equal bytes in
//! independent allocations stay distinguishable, as the C test's pointers do.
use obs_output::output_get_last_error::{LastErrorFields, last_error};
use obs_output::output_slots::{MAX_OUTPUT_AUDIO_ENCODERS, MAX_OUTPUT_VIDEO_ENCODERS};

const V: usize = MAX_OUTPUT_VIDEO_ENCODERS;
const A: usize = MAX_OUTPUT_AUDIO_ENCODERS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Encoder {
    Video(usize),
    Audio(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Read {
    Own,
    Video(usize),
    Audio(usize),
    Error(Encoder),
}

/// One allocation: its id and bytes (bytes are never compared by the core).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Error {
    id: u32,
    bytes: &'static [u8],
}

struct Model {
    next: u32,
    own: Option<Error>,
    video_errors: [Option<Error>; V],
    audio_errors: [Option<Error>; A],
    video_slots: [bool; V],
    audio_slots: [bool; A],
    reads: Vec<Read>,
}

impl Model {
    fn new() -> Self {
        Self {
            next: 0,
            own: None,
            video_errors: [None; V],
            audio_errors: [None; A],
            video_slots: [false; V],
            audio_slots: [false; A],
            reads: Vec::new(),
        }
    }
    /// `obs_*_set_last_error()`: a fresh allocation, or NULL.
    fn alloc(&mut self, bytes: Option<&'static [u8]>) -> Option<Error> {
        bytes.map(|bytes| {
            self.next += 1;
            Error {
                id: self.next,
                bytes,
            }
        })
    }
    fn set_own(&mut self, bytes: Option<&'static [u8]>) {
        self.own = self.alloc(bytes);
    }
    fn set_video(&mut self, idx: usize, bytes: Option<&'static [u8]>) {
        self.video_errors[idx] = self.alloc(bytes);
    }
    fn set_audio(&mut self, idx: usize, bytes: Option<&'static [u8]>) {
        self.audio_errors[idx] = self.alloc(bytes);
    }
}

impl LastErrorFields for Model {
    type Encoder = Encoder;
    type Error = Error;
    fn own(&mut self) -> Option<Error> {
        self.reads.push(Read::Own);
        self.own
    }
    fn video(&mut self, idx: usize) -> Option<Encoder> {
        self.reads.push(Read::Video(idx));
        self.video_slots[idx].then_some(Encoder::Video(idx))
    }
    fn audio(&mut self, idx: usize) -> Option<Encoder> {
        self.reads.push(Read::Audio(idx));
        self.audio_slots[idx].then_some(Encoder::Audio(idx))
    }
    fn encoder_error(&mut self, encoder: Encoder) -> Option<Error> {
        self.reads.push(Read::Error(encoder));
        match encoder {
            Encoder::Video(idx) => self.video_errors[idx],
            Encoder::Audio(idx) => self.audio_errors[idx],
        }
    }
}

/// The reads the C makes for this model, ending at the hit if there is one.
fn priority_reads(model: &Model) -> Vec<Read> {
    let mut reads = vec![Read::Own];
    if model.own.is_some() {
        return reads;
    }
    for idx in 0..V {
        reads.push(Read::Video(idx));
        if model.video_slots[idx] {
            reads.push(Read::Error(Encoder::Video(idx)));
            if model.video_errors[idx].is_some() {
                return reads;
            }
        }
    }
    for idx in 0..A {
        reads.push(Read::Audio(idx));
        if model.audio_slots[idx] {
            reads.push(Read::Error(Encoder::Audio(idx)));
            if model.audio_errors[idx].is_some() {
                return reads;
            }
        }
    }
    reads
}

/// `check_query()`: the borrowed result, no diagnostic for a valid output,
/// and every field read once, in priority order, with nothing after the hit.
fn check_query(model: &mut Model, pick: impl Fn(&Model) -> Option<Error>) {
    let expected = pick(model);
    model.reads.clear();
    let before = (
        model.own,
        model.video_errors,
        model.audio_errors,
        model.video_slots,
        model.audio_slots,
    );
    let mut logs = 0;
    let actual = last_error(Some(&mut *model), || logs += 1);
    assert_eq!(actual, expected);
    assert_eq!(logs, 0);
    assert_eq!(model.reads, priority_reads(model));
    let after = (
        model.own,
        model.video_errors,
        model.audio_errors,
        model.video_slots,
        model.audio_slots,
    );
    assert_eq!(after, before, "the query changed a field");
}

#[test]
fn null_diagnostic() {
    let mut logs = 0;
    assert_eq!(last_error::<Model>(None, || logs += 1), None);
    assert_eq!(logs, 1);
}

#[test]
fn fallback_order_and_borrowing() {
    let mut m = Model::new();
    check_query(&mut m, |_| None);
    // Every slot attached; present encoders with NULL errors are skipped.
    m.video_slots = [true; V];
    m.audio_slots = [true; A];
    check_query(&mut m, |_| None);
    // Equal bytes in independent allocations.
    for idx in 0..A {
        m.set_audio(idx, Some(b"same error"));
    }
    for idx in 0..V {
        m.set_video(idx, Some(b"same error"));
    }
    m.set_own(Some(b"same error"));
    check_query(&mut m, |m| m.own);
    // Empty strings are errors, not fallback.
    m.set_own(Some(b""));
    check_query(&mut m, |m| m.own);
    m.set_own(None);
    for idx in 0..V {
        check_query(&mut m, |m| m.video_errors[idx]);
        m.set_video(idx, Some(b""));
        check_query(&mut m, |m| m.video_errors[idx]);
        m.set_video(idx, None);
        let next = if idx + 1 < V {
            m.video_errors[idx + 1]
        } else {
            m.audio_errors[0]
        };
        check_query(&mut m, |_| next);
        m.video_slots[idx] = false;
    }
    for idx in 0..A {
        check_query(&mut m, |m| m.audio_errors[idx]);
        m.set_audio(idx, Some(b""));
        check_query(&mut m, |m| m.audio_errors[idx]);
        m.set_audio(idx, None);
        let next = if idx + 1 < A {
            m.audio_errors[idx + 1]
        } else {
            None
        };
        check_query(&mut m, |_| next);
        m.audio_slots[idx] = false;
    }
    check_query(&mut m, |_| None);
    // Last-slot errors with holes before them, including non-UTF-8 bytes.
    let bytes: &'static [u8] = &[0xff, 0x80, b'x'];
    m.video_slots[V - 1] = true;
    m.audio_slots[A - 1] = true;
    m.set_video(V - 1, Some(bytes));
    m.set_audio(A - 1, Some(bytes));
    // The core has no lock to take: the C test's held encoder mutexes
    // cannot be reached from here, so no added locking is possible.
    check_query(&mut m, |m| m.video_errors[V - 1]);
    assert_eq!(m.video_errors[V - 1].map(|e| e.bytes), Some(bytes));
    m.video_slots[V - 1] = false;
    check_query(&mut m, |m| m.audio_errors[A - 1]);
    m.audio_slots[A - 1] = false;
    check_query(&mut m, |_| None);
}

#[test]
fn encoder_errors_without_an_attached_slot_are_never_read() {
    let mut m = Model::new();
    for idx in 0..V {
        m.set_video(idx, Some(b"detached"));
    }
    for idx in 0..A {
        m.set_audio(idx, Some(b"detached"));
    }
    check_query(&mut m, |_| None);
    assert!(!m.reads.iter().any(|r| matches!(r, Read::Error(_))));
}
