//! Tier 1 mirrors test_output_get_name.c on the safe core, with a cell
//! standing in for the output's context-name slot.
use obs_output::output_get_name::get_name;
use std::cell::Cell;
use std::ffi::CStr;

#[test]
fn null_output_logs_and_returns_null() {
    let logs = Cell::new(0);
    let name = get_name(None::<fn() -> &'static CStr>, || logs.set(logs.get() + 1));
    assert_eq!(name, None);
    assert_eq!(logs.get(), 1);
}

fn name<'a>(slot: &'a Cell<&'a CStr>) -> &'a CStr {
    get_name(Some(|| slot.get()), || panic!("a live output logged")).unwrap()
}

#[test]
fn returns_the_context_name_pointer_silently() {
    let slot = Cell::new(c"get name");
    let first = name(&slot);
    let again = name(&slot);
    assert_eq!(first.as_ptr(), slot.get().as_ptr());
    assert_eq!(again.as_ptr(), first.as_ptr());
    assert_eq!(first, c"get name");
}

#[test]
fn unnamed_output_returns_its_generated_name() {
    let slot = Cell::new(c"__unnamed0003");
    let current = name(&slot);
    assert_eq!(current.as_ptr(), slot.get().as_ptr());
    assert!(current.to_bytes().starts_with(b"__unnamed"));
}

#[test]
fn reads_the_current_context_name() {
    let slot = Cell::new(c"before");
    assert_eq!(name(&slot), c"before");
    let replacement = c"after";
    slot.set(replacement);
    let current = name(&slot);
    assert_eq!(current.as_ptr(), replacement.as_ptr());
    assert_eq!(current, c"after");
}

#[test]
fn projects_once_and_returns_even_an_absent_name() {
    let calls = Cell::new(0);
    let current = get_name(
        Some(|| {
            calls.set(calls.get() + 1);
            None::<&CStr>
        }),
        || panic!("a live output logged"),
    );
    assert_eq!(current, Some(None));
    assert_eq!(calls.get(), 1);
}
