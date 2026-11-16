//! Tier 1: assertions from test_output_type_info.c, with safe borrowed identities.
//! Registration/ID selection is a fixture; the real lookup is checked in Tier 3.
use obs_output::output_type_info::{self as core, Info};
use std::cell::{Cell, RefCell};

type Text<'a> = Option<&'a RefCell<Vec<u8>>>;
struct Trace<'a> {
    calls: Cell<usize>,
    data: Cell<Option<usize>>,
    next: Cell<Text<'a>>,
}
impl Trace<'_> {
    fn new() -> Self {
        Self {
            calls: Cell::new(0),
            data: Cell::new(None),
            next: Cell::new(None),
        }
    }
}
struct Entry<'a> {
    id: &'a [u8],
    flags: u32,
    data: Option<usize>,
    video: Text<'a>,
    audio: Text<'a>,
    trace: &'a Trace<'a>,
    forbidden: bool,
}
impl<'a> Info for Entry<'a> {
    type Text = Text<'a>;
    fn name(&self) -> Self::Text {
        assert!(!self.forbidden, "later duplicate callback called");
        self.trace.calls.set(self.trace.calls.get() + 1);
        self.trace.data.set(self.data);
        self.trace.next.get()
    }
    fn flags(&self) -> u32 {
        self.flags
    }
    fn video_codecs(&self) -> Self::Text {
        self.video
    }
    fn audio_codecs(&self) -> Self::Text {
        self.audio
    }
}
fn entry<'a>(id: &'a [u8], trace: &'a Trace<'a>) -> Entry<'a> {
    Entry {
        id,
        flags: 0,
        data: None,
        video: None,
        audio: None,
        trace,
        forbidden: false,
    }
}
fn bytes(id: &[u8]) -> &[u8] {
    id.split(|b| *b == 0).next().unwrap()
}
fn find<'r, 'a>(entries: &'r [Entry<'a>], id: Option<&[u8]>) -> Option<&'r Entry<'a>> {
    let id = id?;
    entries.iter().find(|entry| bytes(entry.id) == bytes(id))
}
fn identity(actual: Text<'_>, expected: Text<'_>) {
    match (actual, expected) {
        (None, None) => (),
        (Some(a), Some(b)) => assert!(std::ptr::eq(a, b)),
        _ => panic!("borrowed identity differs"),
    }
}
fn metadata(
    info: Option<&Entry<'_>>,
    flags: u32,
    video: Text<'_>,
    audio: Text<'_>,
    trace: &Trace<'_>,
) {
    let before = trace.calls.get();
    assert_eq!(core::flags(info), flags);
    identity(core::video_codecs(info).flatten(), video);
    identity(core::audio_codecs(info).flatten(), audio);
    assert_eq!(trace.calls.get(), before);
}
fn missing(entries: &[Entry<'_>], id: Option<&[u8]>, trace: &Trace<'_>) {
    let before = trace.calls.get();
    let info = find(entries, id);
    metadata(info, 0, None, None, trace);
    identity(core::display_name(info).flatten(), None);
    assert_eq!(trace.calls.get(), before);
}
#[test]
fn missing_and_null_ids() {
    let trace = Trace::new();
    let mut entries = vec![];
    for id in [None, Some(b"".as_slice()), Some(b"unknown-output-type")] {
        missing(&entries, id, &trace);
    }
    let mut known = entry(b"known-output", &trace);
    known.flags = u32::MAX;
    entries.push(known);
    for id in [
        None,
        Some(b"".as_slice()),
        Some(b"unknown-output-type"),
        Some(b"Known-output"),
        Some(b"known-output-suffix"),
        Some(b"known-outpu"),
    ] {
        missing(&entries, id, &trace);
    }
}
#[test]
fn name_callback_argument_and_result() {
    let trace = Trace::new();
    let first = RefCell::new(b"same display name".to_vec());
    let second = RefCell::new(b"same display name".to_vec());
    let empty = RefCell::new(vec![]);
    let raw = RefCell::new(b"n\xff\0x\0".to_vec());
    assert!(!std::ptr::eq(&first, &second));
    let mut info = entry(b"callback-output", &trace);
    info.data = Some(17);
    for result in [
        Some(&first),
        Some(&second),
        Some(&empty),
        Some(&raw),
        None,
        Some(&first),
    ] {
        trace.next.set(result);
        let before = trace.calls.get();
        identity(core::display_name(Some(&info)).flatten(), result);
        assert_eq!(trace.calls.get(), before + 1);
        assert_eq!(trace.data.get(), Some(17));
    }
    let null_data = entry(b"callback-null-data", &trace);
    trace.next.set(Some(&second));
    identity(
        core::display_name(Some(&null_data)).flatten(),
        Some(&second),
    );
    assert_eq!(trace.calls.get(), 7);
    assert_eq!(trace.data.get(), None);
    metadata(Some(&info), 0, None, None, &trace);
}
#[test]
fn exact_flags_and_borrowed_codec_strings() {
    let trace = Trace::new();
    let video = RefCell::new(b" H264;;hevc;H264;unknown ".to_vec());
    let audio = RefCell::new(b"aac;opus;;aac;".to_vec());
    let empty = RefCell::new(vec![]);
    let raw = RefCell::new(b"v\xfe;\0x\0".to_vec());
    let mut info = entry(b"metadata-output", &trace);
    info.video = Some(&video);
    info.audio = Some(&audio);
    for flags in (0..32).map(|bit| 1u32 << bit).chain([u32::MAX, 0]) {
        info.flags = flags;
        metadata(Some(&info), flags, Some(&video), Some(&audio), &trace);
    }
    assert_eq!(*video.borrow(), b" H264;;hevc;H264;unknown ");
    assert_eq!(*audio.borrow(), b"aac;opus;;aac;");
    video.borrow_mut()[0] = b'x';
    audio.borrow_mut()[0] = b'y';
    metadata(Some(&info), 0, Some(&video), Some(&audio), &trace);
    assert_eq!(
        core::video_codecs(Some(&info)).flatten().unwrap().borrow()[0],
        b'x'
    );
    assert_eq!(
        core::audio_codecs(Some(&info)).flatten().unwrap().borrow()[0],
        b'y'
    );
    video.borrow_mut()[0] = b' ';
    audio.borrow_mut()[0] = b'a';
    let codecs = [None, Some(&empty), Some(&raw), Some(&video), Some(&audio)];
    for (i, value) in codecs.iter().enumerate() {
        info.video = *value;
        info.audio = codecs[4 - i];
        metadata(Some(&info), 0, *value, codecs[4 - i], &trace);
    }
}
#[test]
fn first_match_and_byte_exact_ids() {
    let trace = Trace::new();
    let first_video = RefCell::new(b"h264".to_vec());
    let second_video = RefCell::new(b"h264".to_vec());
    let first_audio = RefCell::new(b"aac".to_vec());
    let second_audio = RefCell::new(b"aac".to_vec());
    let result = RefCell::new(b"first registered match".to_vec());
    let mut entries = vec![];
    for (id, flags) in [
        (b"prefix".as_slice(), 1),
        (b"duplicate", 0x80000045),
        (b"duplicate", 2),
        (b"duplicate-suffix", 4),
        (b"o\xff\0", 8),
        (b"", 16),
    ] {
        let mut info = entry(id, &trace);
        info.flags = flags;
        if flags == 0x80000045 || flags == 2 {
            let first = flags != 2;
            info.data = Some(if first { 1 } else { 2 });
            info.video = Some(if first { &first_video } else { &second_video });
            info.audio = Some(if first { &first_audio } else { &second_audio });
            info.forbidden = !first;
        }
        // Public registration rejects a duplicate; direct duplicate arrays are Tier 3.
        if find(&entries, Some(id)).is_none() {
            entries.push(info);
        }
    }
    trace.next.set(Some(&result));
    for i in 0..3 {
        let info = find(&entries, Some(b"duplicate\0ignored bytes"));
        metadata(
            info,
            0x80000045,
            Some(&first_video),
            Some(&first_audio),
            &trace,
        );
        identity(core::display_name(info).flatten(), Some(&result));
        assert_eq!(trace.data.get(), Some(1));
        assert_eq!(trace.calls.get(), i + 1);
    }
    assert!(!std::ptr::eq(&first_video, &second_video));
    assert!(!std::ptr::eq(&first_audio, &second_audio));
    for (id, flags) in [
        (b"prefix".as_slice(), 1),
        (b"duplicate-suffix", 4),
        (b"o\xff\0x\0", 8),
        (b"", 16),
    ] {
        let info = find(&entries, Some(id));
        metadata(info, flags, None, None, &trace);
        if flags == 8 || flags == 16 {
            identity(core::display_name(info).flatten(), Some(&result));
            assert_eq!(trace.data.get(), None);
        }
    }
    assert_eq!(trace.calls.get(), 5);
    for id in [
        Some(b"Duplicate".as_slice()),
        Some(b"duplicat"),
        Some(b"o"),
        None,
    ] {
        missing(&entries, id, &trace);
    }
}
#[test]
fn null_ids_without_core() {
    let trace = Trace::new();
    // Before startup and again after shutdown, no core/entry is accessed.
    for _ in 0..2 {
        missing(&[], None, &trace);
    }
}
