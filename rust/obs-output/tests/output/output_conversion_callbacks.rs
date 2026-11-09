//! Mirrors test_output_conversion_callbacks.c without FFI types.
use obs_output::output_conversion_callbacks::{
    Rejection, conversion_guard, insertion_index, removal_index, store,
};

#[test]
fn conversion_guards() {
    for media in [1, 2] {
        assert_eq!(
            conversion_guard(false, false, 0, media),
            Err(Rejection::Output)
        );
        assert_eq!(
            conversion_guard(false, true, 0, media),
            Err(Rejection::Output)
        );
        for flags in [0, 1, 2, 3, 4, 7, u32::MAX] {
            assert_eq!(
                conversion_guard(true, false, flags, media),
                Err(Rejection::Conversion)
            );
            let expected = if flags & 4 != 0 {
                Err(Rejection::Encoded)
            } else if flags & media == 0 {
                Err(Rejection::Media)
            } else {
                Ok(())
            };
            assert_eq!(conversion_guard(true, true, flags, media), expected);
            if expected.is_err() {
                assert_eq!(
                    store(
                        true,
                        true,
                        flags,
                        media,
                        || -> () { panic!("rejected conversion must not be read") },
                        |_| panic!("rejected output must not be modified")
                    ),
                    expected
                );
            }
        }
    }
}

#[test]
fn conversion_copies() {
    for (media, mut input) in [(1, vec![1, 641, 359, 2, 2]), (2, vec![44100, 8, 6, 0])] {
        for flags in [media, 3] {
            let mut stored = None;
            store(
                true,
                true,
                flags,
                media,
                || input.clone(),
                |value| stored = Some(value),
            )
            .unwrap();
            let before = stored.clone();
            input[0] = 0;
            assert_eq!(stored, before);
            input.fill(0);
            store(
                true,
                true,
                flags,
                media,
                || input.clone(),
                |value| stored = Some(value),
            )
            .unwrap();
            assert_eq!(stored, Some(input.clone()));
        }
    }
}

#[test]
fn packet_callbacks() {
    let mut entries = Vec::new();
    assert_eq!(removal_index(&entries, &(1, 1)), None);
    for entry in [(1, 1), (2, 1), (1, 2), (1, 1)] {
        entries.insert(insertion_index(), entry);
    }
    assert_eq!(entries, [(1, 1), (1, 2), (2, 1), (1, 1)]);
    entries.remove(removal_index(&entries, &(1, 1)).unwrap());
    assert_eq!(entries, [(1, 2), (2, 1), (1, 1)]);
    assert_eq!(removal_index(&entries, &(2, 2)), None);
    for entry in [(1, 2), (1, 1), (2, 1)] {
        entries.remove(removal_index(&entries, &entry).unwrap());
    }
    entries.insert(insertion_index(), (0, 0));
    assert_eq!(entries, [(0, 0)]);
    entries.remove(removal_index(&entries, &(0, 0)).unwrap());
    assert!(entries.is_empty());
}
