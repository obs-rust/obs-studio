//! Tier 1 mirrors test_source_filter_index.c through the safe API.
use obs_source::filter_index::{index, valid};

#[test]
fn invalid_arguments() {
    let mut logs = Vec::new();
    for (source, filter) in [(false, true), (false, false), (true, false)] {
        assert!(!valid(source, filter, |parameter| logs.push(parameter)));
    }
    assert_eq!(logs, [c"source", c"source", c"filter"]);
    assert!(valid(true, true, |_| panic!("valid arguments logged")));
}

#[test]
fn ordered_lookup() {
    let items = [3usize, 1, 2, 1];
    let before = items;
    assert_eq!(index::<usize>(&[], &1), -1);
    assert_eq!(index(&items, &3), 0);
    assert_eq!(index(&items, &2), 2);
    assert_eq!(index(&items, &1), 1);
    assert_eq!(index(&items, &4), -1);
    assert_eq!(index(&items, &5), -1);
    assert_eq!(items, before);
    assert_eq!(items.len(), 4);
    assert_eq!(index(&[7], &7), 0);
    assert_eq!(index(&[1, 2, 3, 4], &4), 3);
}
