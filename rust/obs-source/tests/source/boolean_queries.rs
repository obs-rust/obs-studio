use obs_source::boolean_queries::{is_hidden, texcoords_centered};

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
