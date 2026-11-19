//! Tier 1 mirrors test_service_find.c::registry_lookup without FFI.
use obs_output::service_find::find;

#[derive(Clone, Debug, PartialEq)]
struct Entry {
    id: Vec<u8>,
    owner: usize,
}

fn lookup<'a>(entries: &'a [Entry], count: usize, query: &[u8]) -> Option<&'a Entry> {
    find(query, entries[..count].iter().map(|entry| (entry.id.as_slice(), entry)))
}

#[test]
fn registry_lookup() {
    for query in [b"".as_slice(), b"missing"] {
        assert!(lookup(&[], 0, query).is_none());
    }
    let ids: &[&[u8]] = &[b"svc", b"svc.suffix", b"SVC", b"", b"\xff\x80x", b"svc", b"last", b"outside"];
    let mut entries: Vec<_> = ids.iter().enumerate().map(|(owner, id)| Entry { id: id.to_vec(), owner }).collect();
    assert!(lookup(&entries, 0, b"svc").is_none());
    let queries: &[(&[u8], Option<usize>)] = &[
        (b"svc", Some(0)), (b"sv", None), (b"svc.suffix", Some(1)),
        (b"svc.suffix.more", None), (b"SVC", Some(2)), (b"Svc", None),
        (b"", Some(3)), (b"\xff\x80x", Some(4)), (b"\xff\x81x", None),
        (b"last", Some(6)), (b"outside", None), (b"missing", None),
    ];
    let before = entries.clone();
    for &(query, expected) in queries {
        let actual = lookup(&entries, 7, query);
        assert_eq!(actual.map(|entry| entry.owner), expected);
        if let Some(index) = expected {
            assert!(std::ptr::eq(actual.unwrap(), &entries[index]));
        }
        assert_eq!(entries, before);
    }
    entries.swap(0, 5);
    assert!(std::ptr::eq(lookup(&entries, 7, b"svc").unwrap(), &entries[0]));
    entries[0].id = b"replaced".to_vec();
    assert!(std::ptr::eq(lookup(&entries, 7, b"svc").unwrap(), &entries[5]));
    assert_eq!(entries[5].id, b"svc");
    assert_eq!(entries[0].owner, 5);
}

#[test]
fn stops_before_reading_later_entries() {
    let mut reads = 0;
    let entries = [b"svc".as_slice(), b"svc", b"other"];
    let found = find(b"svc", entries.iter().enumerate().map(|(index, id)| {
        reads += 1;
        (*id, index)
    }));
    assert_eq!(found, Some(0));
    assert_eq!(reads, 1);
}
