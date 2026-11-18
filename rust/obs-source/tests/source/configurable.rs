//! Tier 1 mirrors of test_source_configurable.c.
use obs_source::configurable::{Source, configurable};
use std::cell::Cell;

fn query(source: Option<Source>, logs: &Cell<u32>) -> bool {
    configurable(source, |name| {
        assert_eq!(name.to_bytes(), b"obs_source_configurable");
        logs.set(logs.get() + 1);
    })
}

#[test]
fn test_source_configurable_null_source() {
    let logs = Cell::new(0);
    assert!(!query(None, &logs));
    assert_eq!(logs.get(), 1);
}

/// Without instance data the answer is false whatever callbacks exist, silently.
#[test]
fn test_source_configurable_no_data() {
    let logs = Cell::new(0);
    for (get_properties, get_properties2) in
        [(true, true), (false, true), (true, false), (false, false)]
    {
        let source = Source {
            data: false,
            get_properties,
            get_properties2,
        };
        assert!(!query(Some(source), &logs));
    }
    assert_eq!(logs.get(), 0);
}

/// With data, either callback makes the source configurable.
#[test]
fn test_source_configurable_callback_presence() {
    let logs = Cell::new(0);
    for get_properties in [false, true] {
        for get_properties2 in [false, true] {
            let source = Source {
                data: true,
                get_properties,
                get_properties2,
            };
            assert_eq!(
                query(Some(source), &logs),
                get_properties || get_properties2
            );
        }
    }
    assert_eq!(logs.get(), 0);
}

/// The answer comes from the instance's own callbacks alone (no registry lookup).
#[test]
fn test_source_configurable_reads_own_info() {
    let logs = Cell::new(0);
    let mut source = Source {
        data: true,
        get_properties: false,
        get_properties2: false,
    };
    assert!(!query(Some(source), &logs));
    source = Source {
        data: true,
        get_properties: false,
        get_properties2: true,
    };
    assert!(query(Some(source), &logs));
    assert_eq!(logs.get(), 0);
}
