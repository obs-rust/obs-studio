//! Tier 1 mirrors every metadata assertion in test_source_type_info.c.
use obs_source::type_info::{self, Info};
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
    flags: u32,
    mask: u8,
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
    fn flags(&self) -> u32 {
        self.flags
    }
    fn has_properties(&self) -> bool {
        self.mask & 1 != 0
    }
    fn has_properties2(&self) -> bool {
        self.mask & 2 != 0
    }
}
fn entry<'a>(names: &'a Names<'a>, id: &'a [u8]) -> Entry<'a> {
    Entry {
        id,
        flags: 0,
        mask: 0,
        data: None,
        names,
    }
}
fn query<'a>(entries: &'a [Entry<'a>], id: &[u8]) -> Option<&'a Entry<'a>> {
    entries
        .iter()
        .find(|e| e.id.split(|b| *b == 0).next() == id.split(|b| *b == 0).next())
}
fn same(actual: Option<&Text>, expected: Option<&Text>) {
    assert!(match (actual, expected) {
        (Some(a), Some(b)) => std::ptr::eq(a, b),
        (None, None) => true,
        _ => false,
    });
}
fn missing(info: Option<&Entry<'_>>, names: &Names<'_>) {
    let before = names.calls.get();
    same(type_info::display_name(info).flatten(), None);
    assert_eq!(type_info::output_flags(info), 0);
    assert!(!type_info::configurable(info));
    assert_eq!(names.calls.get(), before);
}
#[test]
fn test_source_type_info_null_ids_during_empty_registry() {
    let names = Names::default();
    // Lookup of NULL, empty and unknown IDs in a live empty registry is None.
    for _ in 0..3 {
        missing(None, &names);
    }
}
#[test]
fn test_source_type_info_unknown_ids() {
    let names = Names::default();
    let entries = [entry(&names, b"known")];
    for id in [
        b"".as_slice(),
        b"unregistered-source-type",
        b"unknown",
        b"Known",
        b"know",
        b"known-suffix",
    ] {
        missing(query(&[], id), &names);
        missing(query(&entries, id), &names);
    }
}
#[test]
fn test_source_type_info_exact_name_callback_and_borrowed_result() {
    let names = Names::default();
    let first = Text::new(b"Mixed Name".to_vec());
    let second = Text::new(b"Mixed Name".to_vec());
    let empty = Text::new(vec![]);
    let raw = Text::new(vec![0xfe, 0x81, 0]);
    assert!(!std::ptr::eq(&first, &second));
    assert_eq!(first.borrow().as_slice(), second.borrow().as_slice());
    for data in [Some(17), None] {
        let info = Entry {
            data,
            ..entry(&names, b"name")
        };
        for result in [None, Some(&empty), Some(&first), Some(&second), Some(&raw)] {
            names.next.set(result);
            let before = names.calls.get();
            same(type_info::display_name(Some(&info)).flatten(), result);
            assert_eq!(names.calls.get(), before + 1);
            assert_eq!(names.last.get(), data);
            assert_eq!(type_info::output_flags(Some(&info)), 0);
            assert!(!type_info::configurable(Some(&info)));
            assert_eq!(names.calls.get(), before + 1);
        }
    }
    let info = entry(&names, b"name");
    names.next.set(Some(&first));
    let borrowed = type_info::display_name(Some(&info)).flatten().unwrap();
    first.borrow_mut()[0] = b'X';
    assert_eq!(borrowed.borrow()[0], b'X');
    first.borrow_mut()[0] = b'M';
}
#[test]
fn test_source_type_info_exact_flags() {
    let names = Names::default();
    for flags in (0..32)
        .map(|bit| 1 << bit)
        .chain([u32::MAX ^ 0x40, 0, u32::MAX])
    {
        let info = Entry {
            flags,
            ..entry(&names, b"flags")
        };
        assert_eq!(type_info::output_flags(Some(&info)), flags);
    }
    assert_eq!(names.calls.get(), 0);
}
#[test]
fn test_source_type_info_configurable_callback_presence() {
    let names = Names::default();
    // Bits 2/3 model defaults/defaults2; neither implies configurability.
    for mask in 0..16 {
        let info = Entry {
            mask,
            ..entry(&names, b"properties")
        };
        assert_eq!(type_info::configurable(Some(&info)), mask & 3 != 0);
        assert_eq!(type_info::output_flags(Some(&info)), 0);
    }
    assert_eq!(names.calls.get(), 0);
}
#[test]
fn test_source_type_info_first_registration_and_exact_ids() {
    let names = Names::default();
    let result = Text::new(b"first name".to_vec());
    names.next.set(Some(&result));
    // Public registration rejects the second duplicate. Its first-match
    // metadata is also retained when the lookup fixture contains a duplicate.
    let entries = [
        Entry {
            flags: 2,
            mask: 2,
            data: Some(17),
            ..entry(&names, b"first")
        },
        Entry {
            data: Some(99),
            ..entry(&names, b"first")
        },
        Entry {
            flags: 0x8000_0000,
            mask: 1,
            data: Some(99),
            ..entry(&names, b"")
        },
        Entry {
            flags: 0x8000_0000,
            mask: 1,
            data: Some(99),
            ..entry(&names, b"first-suffix")
        },
        Entry {
            flags: 0x8000_0000,
            mask: 1,
            data: Some(99),
            ..entry(&names, b"\xfeid")
        },
    ];
    let first = query(&entries, b"first\0ignored");
    same(type_info::display_name(first).flatten(), Some(&result));
    assert_eq!(names.last.get(), Some(17));
    assert_eq!(names.calls.get(), 1);
    assert_eq!(type_info::output_flags(first), 2);
    assert!(type_info::configurable(first));
    for id in [b"First".as_slice(), b"firs", b"first-suffix-missing"] {
        missing(query(&entries, id), &names);
    }
    for id in [b"".as_slice(), b"first-suffix", b"\xfeid"] {
        let info = query(&entries, id);
        same(type_info::display_name(info).flatten(), Some(&result));
        assert_eq!(names.last.get(), Some(99));
        assert_eq!(type_info::output_flags(info), 0x8000_0000);
        assert!(type_info::configurable(info));
    }
    assert_eq!(type_info::output_flags(query(&entries, b"first")), 2);
    assert_eq!(names.calls.get(), 4);
}
