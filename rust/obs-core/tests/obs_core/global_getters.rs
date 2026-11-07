//! Tier 1: mirrors test_core_global_getters.c through the safe field-read API.
use obs_core::global_getters::borrowed;

#[test]
fn locale_borrow() {
    let mut locale = "en-US";
    let result = borrowed(&locale);
    assert_eq!(result, "en-US");
    assert!(std::ptr::eq(result.as_ptr(), borrowed(&locale).as_ptr()));
    locale = "fr-FR";
    let result = borrowed(&locale);
    assert_eq!(result, "fr-FR");
    assert!(std::ptr::eq(result.as_ptr(), borrowed(&locale).as_ptr()));
}

#[test]
fn signal_borrow() {
    let handler = 1_u8;
    let field = Some(&handler);
    let result = borrowed(&field).unwrap();
    assert!(std::ptr::eq(result, borrowed(&field).unwrap()));
    assert!(std::ptr::eq(result, &handler));
}

#[test]
fn proc_borrow() {
    let handler = 2_u8;
    let field = Some(&handler);
    let result = borrowed(&field).unwrap();
    assert!(std::ptr::eq(result, borrowed(&field).unwrap()));
    assert!(std::ptr::eq(result, &handler));
}

#[test]
fn null_fields_and_replacements() {
    let mut field: Option<&u8> = None;
    assert_eq!(borrowed(&field), None);
    let first = 3;
    let second = 4;
    field = Some(&first);
    assert!(std::ptr::eq(borrowed(&field).unwrap(), &first));
    field = Some(&second);
    assert!(std::ptr::eq(borrowed(&field).unwrap(), &second));
    field = None;
    assert_eq!(borrowed(&field), None);
}
