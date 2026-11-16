use obs_source::{
    boolean_queries::{is_hidden, texcoords_centered},
    hidden_setter::set_hidden,
};

#[test]
fn mirrors_c_test_source_boolean_queries_independent_fields() {
    for centered in [false, true] {
        for hidden in [false, true] {
            let mut source = (centered, !hidden, true, 0xa5a5_5a5a_u32);
            set_hidden(&mut source.1, hidden);
            let before = source;
            assert_eq!(texcoords_centered(&source.0), centered);
            assert_eq!(source, before);
            assert_eq!(is_hidden(&source.1), hidden);
            assert_eq!(source, before);
            assert_eq!(texcoords_centered(&source.0), centered);
            assert_eq!(is_hidden(&source.1), hidden);
            assert_eq!(source, before);
            assert_eq!(source, (centered, hidden, true, 0xa5a5_5a5a));
        }
    }
}

#[test]
fn repeated_assignments_replace_previous_state() {
    for initial in [false, true] {
        let mut current = initial;
        for hidden in [initial, !initial, !initial, initial] {
            set_hidden(&mut current, hidden);
            assert_eq!(current, hidden);
        }
    }
}
