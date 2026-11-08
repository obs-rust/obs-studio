//! Mirrors test_module_name_lookups, including first-match identity and no writes.
use obs_core::module_name_lookups::lookup;

#[test]
fn module_name_lookups() {
    let names: [&[u8]; 4] = [b"alpha", b"", b"alpha", b"x\xff"];
    for _disabled in [false, true] {
        let query = |first, name: &[u8]| {
            lookup(first, name, |i: usize| (names[i], (i + 1 < names.len()).then_some(i + 1)))
        };
        assert_eq!(query(None, b"alpha"), None);
        for (name, expected) in [(b"alpha".as_slice(), Some(0)), (b"", Some(1)), (b"x\xff", Some(3)), (b"Alpha", None), (b"alph", None), (b"missing", None), (b"alpha", Some(0))] {
            assert_eq!(query(Some(0), name), expected);
        }
        assert_eq!(names, [b"alpha".as_slice(), b"", b"alpha", b"x\xff"]);
    }
}
