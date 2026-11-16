//! Tier 1: `test_source_private_settings.c` against the safe core. The model
//! source holds a reference-counted stand-in for its `private_settings` and an
//! independent ordinary settings value the getter must never touch.
use obs_source::private_settings::{NAME, get};

/// A reference-counted data handle: `0` is NULL, any other id names an object.
#[derive(Default)]
struct Data {
    refs: Vec<usize>,
}

impl Data {
    fn create(&mut self) -> usize {
        self.refs.push(1);
        self.refs.len()
    }
    /// `obs_data_addref` accepts NULL and does nothing for it.
    fn addref(&mut self, id: usize) {
        if id != 0 {
            self.refs[id - 1] += 1;
        }
    }
    fn release(&mut self, id: usize) {
        if id != 0 {
            self.refs[id - 1] -= 1;
        }
    }
    fn refs(&self, id: usize) -> usize {
        self.refs[id - 1]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Source {
    private_settings: usize,
    settings: usize,
}

fn get_private_settings(
    data: &mut Data,
    source: Option<&Source>,
    logs: &mut Vec<&'static core::ffi::CStr>,
) -> usize {
    get(
        source.map(|source| source.private_settings),
        |id| data.addref(id),
        |name| logs.push(name),
    )
}

#[test]
fn test_source_private_settings_null_and_missing_settings() {
    let mut data = Data::default();
    let mut logs = Vec::new();
    let null_result = get_private_settings(&mut data, None, &mut logs);
    let source = Source::default();
    let before = source;
    let missing_result = get_private_settings(&mut data, Some(&source), &mut logs);

    assert_eq!(null_result, 0);
    assert_eq!(missing_result, 0);
    // One LOG_DEBUG line, for the NULL source only, under the C's name.
    assert_eq!(logs, [NAME]);
    assert_eq!(NAME, c"obs_source_get_private_settings");
    // Characterized, not endorsed: a stored NULL is not replaced or diagnosed.
    assert_eq!(source, before);
}

#[test]
fn test_source_private_settings_retained_identity() {
    for count in [1, 2, 5] {
        let mut data = Data::default();
        let mut logs = Vec::new();
        let source = Source {
            private_settings: data.create(),
            settings: data.create(),
        };
        let before = source;
        let mut retained = Vec::new();
        for i in 0..count {
            retained.push(get_private_settings(&mut data, Some(&source), &mut logs));
            assert_eq!(retained[i], source.private_settings);
            // Exactly one retain per call, none on the ordinary settings.
            assert_eq!(data.refs(source.private_settings), 2 + i);
            assert_eq!(data.refs(source.settings), 1);
            assert_eq!(source, before);
        }
        assert!(logs.is_empty());

        // The source drops its own reference; the returned handles keep the
        // object alive until the last one is released, and no longer.
        data.release(source.private_settings);
        for (i, id) in retained.iter().enumerate() {
            assert_eq!(data.refs(*id), count - i);
            data.release(*id);
        }
        assert_eq!(data.refs(source.private_settings), 0);
        assert_eq!(data.refs(source.settings), 1);
    }
}

#[test]
fn stored_null_is_returned_with_one_null_addref() {
    let mut calls = Vec::new();
    let value = get(Some(0usize), |id| calls.push(id), |_| panic!("live source"));
    assert_eq!(value, 0);
    assert_eq!(calls, [0]);
}

#[test]
fn null_source_logs_without_addref() {
    let mut logs = 0;
    let value = get::<usize>(
        None,
        |_| panic!("NULL source"),
        |name| {
            assert_eq!(name, NAME);
            logs += 1;
        },
    );
    assert_eq!(value, 0);
    assert_eq!(logs, 1);
}

#[test]
fn identity_over_any_value() {
    for stored in [1, 2, usize::MAX / 2, usize::MAX] {
        let mut calls = Vec::new();
        let value = get(Some(stored), |id| calls.push(id), |_| panic!("live source"));
        assert_eq!(value, stored);
        assert_eq!(calls, [stored]);
    }
}
