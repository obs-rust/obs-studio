//! Tier 1 mirrors test_encoder_get_name.c without a native encoder layout.
use obs_encoder::get_name::get;
use std::cell::Cell;
use std::ffi::CStr;

#[test]
fn null_encoder_logs_and_returns_null() {
    let mut logs = Vec::new();
    assert_eq!(
        get::<usize>(None::<fn() -> usize>, |level, text| logs
            .push((level, text))),
        None
    );
    assert_eq!(
        logs,
        [(400, c"obs_encoder_get_name: Null 'encoder' parameter")]
    );
}

fn name<'a>(value: &'a Cell<&'a CStr>) -> &'a CStr {
    get(Some(|| value.get()), |_, _| panic!("valid encoder logged")).unwrap()
}

#[test]
fn returns_the_context_name_pointer_silently() {
    let value = Cell::new(c"get name");
    let first = name(&value);
    assert_eq!(first.as_ptr(), value.get().as_ptr());
    assert_eq!(name(&value).as_ptr(), first.as_ptr());
    assert_eq!(first, c"get name");
}

#[test]
fn unnamed_encoder_returns_its_generated_name() {
    let value = Cell::new(c"__unnamed0017");
    let current = name(&value);
    assert_eq!(current.as_ptr(), value.get().as_ptr());
    assert!(current.to_bytes().starts_with(b"__unnamed"));
}

#[test]
fn follows_a_rename() {
    let value = Cell::new(c"before");
    assert_eq!(name(&value), c"before");
    value.set(c"after");
    let current = name(&value);
    assert_eq!(current.as_ptr(), value.get().as_ptr());
    assert_eq!(current, c"after");
}

#[test]
fn projects_exactly_once_even_for_an_absent_name() {
    let calls = Cell::new(0);
    let current = get(
        Some(|| {
            calls.set(calls.get() + 1);
            None::<usize>
        }),
        |_, _| panic!("valid encoder logged"),
    );
    assert_eq!(current, Some(None));
    assert_eq!(calls.get(), 1);
}
