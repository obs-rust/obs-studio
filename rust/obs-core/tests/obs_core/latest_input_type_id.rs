//! Safe mirrors of test_latest_input_type_id.c; IDs retain their original owners.
use obs_core::latest_input_type_id::latest;
use std::cell::Cell;

#[test]
fn null_id_without_core() {
    assert_eq!(
        latest::<usize>(None, usize::MAX, |_| panic!("NULL reads no registry")),
        None
    );
}

#[test]
fn newest_and_borrowed_pointer() {
    let newest = Cell::new(*b"camera_v9");
    let other = Cell::new(*b"other_v99");
    let entries = [
        (b"camera".as_slice(), 0, &other),
        (b"other", 99, &other),
        (b"camera", 9, &newest),
        (b"camera", 3, &other),
    ];
    let before = entries;
    let query = b"camera".to_vec();
    let result = latest(Some(&query), entries.len(), |i| entries[i]).unwrap();
    assert!(std::ptr::eq(result, &newest));
    assert!(std::ptr::eq(
        latest(Some(b"camera"), entries.len(), |i| entries[i]).unwrap(),
        result
    ));
    newest.set(*b"Camera_v9");
    assert_eq!(result.get(), *b"Camera_v9");
    assert_eq!(entries.len(), 4);
    assert_eq!(entries, before);
    assert!(latest(None, entries.len(), |i| entries[i]).is_none());
}

#[test]
fn ties_keep_first_registered() {
    let mut entries = [(b"camera".as_slice(), 7, "first"), (b"camera", 7, "second")];
    assert_eq!(
        latest(Some(b"camera"), entries.len(), |i| entries[i]),
        Some("first")
    );
    entries.swap(0, 1);
    assert_eq!(
        latest(Some(b"camera"), entries.len(), |i| entries[i]),
        Some("second")
    );
}

#[test]
fn signed_versions() {
    let entries = [
        (b"camera".as_slice(), 0x8000_0000, "negative"),
        (b"camera", 0, "zero"),
        (b"camera", u32::MAX, "sentinel"),
        (b"camera", i32::MAX as u32, "maximum"),
        (b"camera", 0x8000_0001, "negative-last"),
    ];
    assert_eq!(latest(Some(b"camera"), 3, |i| entries[i]), Some("zero"));
    assert_eq!(
        latest(Some(b"camera"), entries.len(), |i| entries[i]),
        Some("maximum")
    );
}

#[test]
fn missing_id_without_assertions() {
    assert_eq!(
        latest::<usize>(Some(b"missing"), 0, |_| unreachable!()),
        None
    );
    let entry = (b"camera".as_slice(), 0, 0);
    assert_eq!(latest(Some(b"missing"), 1, |_| entry), None);
    assert_eq!(
        latest(Some(b"camera"), 1, |_| (b"camera".as_slice(), u32::MAX, 0)),
        None
    );
}

#[test]
fn linked_registry_returns_borrowed_latest() {
    let ids = [
        "latest_input_fixture",
        "latest_input_fixture_v9",
        "latest_input_fixture_v3",
    ];
    let versions = [0, 9, 3];
    for _ in 0..2 {
        let selected = latest(Some(b"latest_input_fixture"), 3, |i| {
            (b"latest_input_fixture".as_slice(), versions[i], &ids[i])
        })
        .unwrap();
        assert_eq!(*selected, "latest_input_fixture_v9");
        assert!(std::ptr::eq(selected, &ids[1]));
    }
    assert!(latest::<usize>(None, 3, |_| unreachable!()).is_none());
}
