use obs_core::module_failure_info::free_table;

// Mirrors cleanup_empty in test_module_failure_info.c (#3854).
#[test]
fn cleanup_empty() {
    let mut table = 0usize;
    free_table(&mut table, |_| panic!("null table must not be freed"));
    assert_eq!(table, 0);
}

// Mirrors cleanup_table_only: exactly one free; repeat is a no-op. No count
// or string data reaches this core, so neither can be traversed or changed.
#[test]
fn cleanup_table_only() {
    let mut table = 17usize;
    let mut freed = Vec::new();
    free_table(&mut table, |value| freed.push(value));
    assert_eq!(table, 0);
    assert_eq!(freed, [17]);
    free_table(&mut table, |value| freed.push(value));
    assert_eq!(table, 0);
    assert_eq!(freed, [17]);
}
