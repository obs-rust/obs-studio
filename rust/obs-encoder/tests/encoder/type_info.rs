//! Tier 1 mirrors test_encoder_type_info.c with safe borrowed values.
use obs_encoder::type_info::{self, Info};
use std::cell::{Cell, RefCell};

type Text = RefCell<Vec<u8>>;
#[derive(Default)]
struct Names<'a> {
    next: Cell<Option<&'a Text>>,
    calls: Cell<usize>,
    last: Cell<Option<usize>>,
}
struct Entry<'a> {
    id: &'a [u8],
    codec: Option<&'a Text>,
    data: Option<usize>,
    names: &'a Names<'a>,
}
impl<'a> Info for Entry<'a> {
    type Text = Option<&'a Text>;
    fn name(&self) -> Self::Text {
        self.names.calls.set(self.names.calls.get() + 1);
        self.names.last.set(self.data);
        self.names.next.get()
    }
    fn codec(&self) -> Self::Text {
        self.codec
    }
}
fn text(bytes: &[u8]) -> Text {
    RefCell::new(bytes.to_vec())
}
fn query<'a>(entries: &'a [Entry<'a>], id: &[u8]) -> Option<&'a Entry<'a>> {
    let c_bytes = |bytes: &'a [u8]| bytes.split(|b| *b == 0).next().unwrap();
    entries
        .iter()
        .find(|e| c_bytes(e.id) == id.split(|b| *b == 0).next().unwrap())
}
fn same(actual: Option<&Text>, expected: Option<&Text>) {
    assert!(match (actual, expected) {
        (Some(a), Some(b)) => std::ptr::eq(a, b),
        (None, None) => true,
        _ => false,
    });
}
fn missing(entry: Option<&Entry<'_>>, names: &Names<'_>) {
    let before = names.calls.get();
    same(type_info::codec(entry).flatten(), None);
    same(type_info::display_name(entry).flatten(), None);
    assert_eq!(names.calls.get(), before);
}
#[test]
fn test_encoder_type_info_missing_and_empty_registry_null_ids() {
    let names = Names::default();
    missing(None, &names); // Empty registry, including the C NULL-ID case.
    let codec = text(b"h264");
    let entries = [Entry {
        id: b"known-encoder",
        codec: Some(&codec),
        data: None,
        names: &names,
    }];
    assert_eq!(entries[0].id, b"known-encoder");
    for id in [
        b"".as_slice(),
        b"unknown-encoder-type",
        b"Known-encoder",
        b"known-encoder-suffix",
        b"known-encode",
    ] {
        missing(query(&entries, id), &names);
    }
}
#[test]
fn test_encoder_type_info_name_callback_argument_and_result() {
    let names = Names::default();
    let first = text(b"same display name");
    let second = text(b"same display name");
    let empty = text(b"");
    let raw = text(b"n\xff\0x\0");
    let codec = text(b"h264");
    let entry = Entry {
        id: b"callback-encoder",
        codec: Some(&codec),
        data: Some(17),
        names: &names,
    };
    assert!(!std::ptr::eq(&first, &second));
    for result in [
        Some(&first),
        Some(&second),
        Some(&empty),
        Some(&raw),
        None,
        Some(&first),
    ] {
        names.next.set(result);
        let before = names.calls.get();
        same(type_info::display_name(Some(&entry)).flatten(), result);
        assert_eq!(names.calls.get(), before + 1);
        assert_eq!(names.last.get(), Some(17));
        same(type_info::codec(Some(&entry)).flatten(), Some(&codec));
        assert_eq!(names.calls.get(), before + 1);
    }
    first.borrow_mut()[0] = b'S';
    assert_eq!(
        type_info::display_name(Some(&entry))
            .flatten()
            .unwrap()
            .borrow()[0],
        b'S'
    );
    assert_eq!(names.calls.get(), 7);
    assert_eq!(names.last.get(), Some(17));
    first.borrow_mut()[0] = b's';
    let null_data = Entry {
        id: b"callback-null-data",
        data: None,
        ..entry
    };
    names.next.set(Some(&second));
    same(
        type_info::display_name(Some(&null_data)).flatten(),
        Some(&second),
    );
    assert_eq!(names.calls.get(), 8);
    assert_eq!(names.last.get(), None);
}
#[test]
fn test_encoder_type_info_exact_borrowed_codec_strings() {
    let names = Names::default();
    let video = text(b" H264;;hevc;H264;unknown ");
    let audio = text(b"aac;opus;;aac;");
    let empty = text(b"");
    let raw = text(b"v\xfe;\0x\0");
    let same_video = text(&video.borrow());
    assert!(!std::ptr::eq(&video, &same_video));
    for (i, codec) in [
        None,
        Some(&empty),
        Some(&raw),
        Some(&video),
        Some(&audio),
        Some(&same_video),
    ]
    .into_iter()
    .enumerate()
    {
        let entry = Entry {
            id: b"codec",
            codec,
            data: None,
            names: &names,
        };
        same(type_info::codec(Some(&entry)).flatten(), codec);
        assert_eq!(names.calls.get(), i);
        names.next.set(Some(&video));
        same(
            type_info::display_name(Some(&entry)).flatten(),
            Some(&video),
        );
        assert_eq!(names.calls.get(), i + 1);
        assert_eq!(names.last.get(), None);
    }
    assert_eq!(&*video.borrow(), b" H264;;hevc;H264;unknown ");
    assert_eq!(&*audio.borrow(), b"aac;opus;;aac;");
    video.borrow_mut()[0] = b'x';
    audio.borrow_mut()[0] = b'y';
    for (value, byte) in [(&video, b'x'), (&audio, b'y')] {
        let entry = Entry {
            id: b"mutable",
            codec: Some(value),
            data: None,
            names: &names,
        };
        let before = names.calls.get();
        same(type_info::codec(Some(&entry)).flatten(), Some(value));
        assert_eq!(
            type_info::codec(Some(&entry)).flatten().unwrap().borrow()[0],
            byte
        );
        assert_eq!(names.calls.get(), before);
    }
}
#[test]
fn test_encoder_type_info_first_match_and_byte_exact_ids() {
    let names = Names::default();
    let first = text(b"h264");
    let second = text(b"h264");
    let result = text(b"first registered match");
    let prefix = text(b"prefix-codec");
    let suffix = text(b"suffix-codec");
    let raw = text(b"raw-id-codec");
    let empty = text(b"empty-id-codec");
    assert!(!std::ptr::eq(&first, &second));
    // Public registration rejects the later duplicate; Tier 2 pins that path.
    let entries = [
        Entry {
            id: b"prefix",
            codec: Some(&prefix),
            data: None,
            names: &names,
        },
        Entry {
            id: b"duplicate",
            codec: Some(&first),
            data: Some(17),
            names: &names,
        },
        Entry {
            id: b"duplicate",
            codec: Some(&second),
            data: Some(99),
            names: &names,
        },
        Entry {
            id: b"duplicate-suffix",
            codec: Some(&suffix),
            data: None,
            names: &names,
        },
        Entry {
            id: b"e\xff\0",
            codec: Some(&raw),
            data: None,
            names: &names,
        },
        Entry {
            id: b"",
            codec: Some(&empty),
            data: None,
            names: &names,
        },
    ];
    names.next.set(Some(&result));
    for i in 0..3 {
        let entry = query(&entries, b"duplicate\0ignored bytes");
        same(type_info::codec(entry).flatten(), Some(&first));
        same(type_info::display_name(entry).flatten(), Some(&result));
        assert_eq!(names.last.get(), Some(17));
        assert_eq!(names.calls.get(), i + 1);
    }
    for (id, codec) in [
        (b"prefix".as_slice(), &prefix),
        (b"duplicate-suffix", &suffix),
        (b"e\xff\0x\0", &raw),
        (b"", &empty),
    ] {
        let entry = query(&entries, id);
        same(type_info::codec(entry).flatten(), Some(codec));
        if id == b"" || id.starts_with(b"e") {
            same(type_info::display_name(entry).flatten(), Some(&result));
            assert_eq!(names.last.get(), None);
        }
    }
    assert_eq!(names.calls.get(), 5);
    for id in [b"Duplicate".as_slice(), b"duplicat", b"e"] {
        missing(query(&entries, id), &names);
    }
}
