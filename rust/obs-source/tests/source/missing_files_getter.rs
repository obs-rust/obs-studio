//! Tier 1 mirrors of test_source_missing_files_getter.c.
use obs_source::missing_files_getter::{Source, get};
use std::cell::Cell;

fn fallback(present: bool, data: bool, installed: bool) {
    let allocations = Cell::new(0);
    let logs = Cell::new(0);
    let mut lists = Vec::new();
    for _ in 0..2 {
        let source = present.then_some(Source {
            data: data.then_some(7),
            callback: installed.then_some(|_| -> Box<Vec<u8>> { panic!("callback must be skipped") }),
        });
        lists.push(get(
            source,
            || {
                allocations.set(allocations.get() + 1);
                Box::new(Vec::<u8>::new())
            },
            |name| {
                assert_eq!(name.to_bytes(), b"obs_source_get_missing_files");
                logs.set(logs.get() + 1);
            },
        ));
    }
    assert!(!std::ptr::eq(&*lists[0], &*lists[1]));
    assert!(lists.iter().all(|list| list.is_empty()));
    assert_eq!(allocations.get(), 2);
    assert_eq!(logs.get(), if present { 0 } else { 2 });
}

#[test]
fn test_source_missing_files_getter_null_source() {
    fallback(false, false, false);
}

#[test]
fn test_source_missing_files_getter_null_data_skips_callback() {
    fallback(true, false, true);
}

#[test]
fn test_source_missing_files_getter_absent_callback() {
    fallback(true, true, false);
}

fn callback_result(result: Option<Box<Vec<u8>>>) {
    let expected = result.as_deref().map(std::ptr::from_ref);
    let calls = Cell::new(0);
    let data = String::from("source context data");
    let source = Source {
        data: Some(&data),
        callback: Some(|received: &String| {
            assert!(std::ptr::eq(received, &data));
            calls.set(calls.get() + 1);
            result
        }),
    };
    let actual = get(Some(source), || panic!("no fallback"), |_| panic!("no log"));
    assert_eq!(actual.as_deref().map(std::ptr::from_ref), expected);
    assert_eq!(calls.get(), 1);
}

#[test]
fn test_source_missing_files_getter_callback_result() {
    callback_result(Some(Box::default()));
}

#[test]
fn test_source_missing_files_getter_null_callback_result() {
    callback_result(None);
}
