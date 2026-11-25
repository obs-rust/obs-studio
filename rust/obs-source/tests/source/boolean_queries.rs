use obs_source::boolean_queries::{is_hidden, set_texcoords_centered, texcoords_centered};

#[test]
fn mirrors_c_independent_fields() {
    for centered in [false, true] {
        for hidden in [false, true] {
            let before = (centered, hidden, true, 0xa5a5_5a5a_u32);
            let source = before;
            assert_eq!(texcoords_centered(&source.0), centered);
            assert_eq!(source, before);
            assert_eq!(is_hidden(&source.1), hidden);
            assert_eq!(source, before);
            assert_eq!(texcoords_centered(&source.0), centered);
            assert_eq!(is_hidden(&source.1), hidden);
            assert_eq!(source, before);
        }
    }
}

/// Tier 1 mirror of test_source_texcoords_centered_setter.c's
/// stores_only_the_flag: each store replaces the flag, whatever it held, and
/// leaves its neighbours alone.
#[test]
fn set_texcoords_centered_stores_only_the_flag() {
    for initial in [false, true] {
        let mut source = (initial, !initial, initial, 0xa5a5_5a5a_u32);
        for requested in [true, true, false, false, true, false] {
            set_texcoords_centered(&mut source.0, requested);
            assert_eq!(source, (requested, !initial, initial, 0xa5a5_5a5a));
        }
    }
}

/// Tier 1 mirror of getter_answers_what_was_stored: the getter answers the
/// stored value and writes nothing.
#[test]
fn getter_answers_what_was_stored() {
    for value in [false, true] {
        let mut source = (!value, !value, !value, 0xa5a5_5a5a_u32);
        set_texcoords_centered(&mut source.0, value);
        let snapshot = source;
        for _ in 0..2 {
            assert_eq!(texcoords_centered(&source.0), value);
            assert_eq!(is_hidden(&source.1), !value);
            assert_eq!(source, snapshot);
        }
    }
}
