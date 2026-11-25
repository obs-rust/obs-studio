//! Tier 1 mirrors of test_source_replace_missing_file.c.
use obs_source::replace_missing_file::replace;
use std::cell::{Cell, RefCell};

struct Run {
    calls: Vec<(usize, Option<&'static str>, Option<usize>)>,
    logs: Vec<Vec<u8>>,
}

/// One call on a model source: `source` is `None` for NULL, `Some(None)`
/// for no private data. The callback records what C hands to `cb`.
fn run(source: Option<Option<usize>>, path: Option<&'static str>, data: Option<usize>) -> Run {
    let calls = RefCell::new(Vec::new());
    let logs = RefCell::new(Vec::new());
    replace(
        source,
        |private| calls.borrow_mut().push((private, path, data)),
        |name| logs.borrow_mut().push(name.to_bytes().to_vec()),
    );
    Run {
        calls: calls.into_inner(),
        logs: logs.into_inner(),
    }
}

#[test]
fn test_source_replace_missing_file_null_source() {
    let got = run(None, Some("new.png"), Some(1));
    assert!(got.calls.is_empty());
    assert_eq!(got.logs, [b"obs_source_replace_missing_file".to_vec()]);
}

#[test]
fn test_source_replace_missing_file_null_data_skips_callback() {
    let got = run(Some(None), Some("new.png"), Some(1));
    assert!(got.calls.is_empty());
    assert!(got.logs.is_empty());
}

#[test]
fn test_source_replace_missing_file_calls_back_with_private_data() {
    let got = run(Some(Some(7)), Some("fixed.png"), Some(9));
    assert_eq!(got.calls, [(7, Some("fixed.png"), Some(9))]);
    assert!(got.logs.is_empty());
}

#[test]
fn test_source_replace_missing_file_passes_null_path_and_data() {
    let got = run(Some(Some(7)), None, None);
    assert_eq!(got.calls, [(7, None, None)]);
    assert!(got.logs.is_empty());
}

#[test]
fn test_source_replace_missing_file_each_call_calls_back_once() {
    let total = Cell::new(0);
    for _ in 0..3 {
        replace(
            Some(Some(7)),
            |_| total.set(total.get() + 1),
            |_| panic!("no log"),
        );
    }
    assert_eq!(total.get(), 3);
}
