//! Tier 1 mirrors of test_source_output_flags.c.
use obs_source::output_flags::output_flags;
use std::cell::Cell;

/// One query over a source whose `info.output_flags` is `info`, counting
/// reads and logs.
fn query(info: Option<u32>, reads: &Cell<u32>, logs: &Cell<u32>) -> u32 {
    let source = info.map(|flags| {
        move || {
            reads.set(reads.get() + 1);
            flags
        }
    });
    output_flags(source, |name| {
        assert_eq!(name.to_bytes(), b"obs_source_get_output_flags");
        logs.set(logs.get() + 1);
    })
}

/// Every pattern and every single bit comes back exactly, read once, silently.
/// Instance `flags` and `default_flags` are never consulted: the core only
/// sees the info value.
#[test]
fn info_flags_are_returned_without_mutation() {
    let (reads, logs) = (Cell::new(0), Cell::new(0));
    let patterns = [0, u32::MAX, 0xaaaa_aaaa, 0x5555_5555];
    let bits = (0..32).map(|bit| 1u32 << bit);
    let mut expected_reads = 0;
    for flags in patterns.into_iter().chain(bits) {
        assert_eq!(query(Some(flags), &reads, &logs), flags);
        expected_reads += 1;
        assert_eq!(reads.get(), expected_reads);
    }
    assert_eq!(logs.get(), 0);
}

/// Two sources answer independently, and a later change of one source's info
/// shows on its next query only.
#[test]
fn independent_sources_and_instance_flags() {
    const VIDEO: u32 = 1 << 0;
    const AUDIO: u32 = 1 << 1;
    const ASYNC_VIDEO: u32 = (1 << 2) | VIDEO;
    let (reads, logs) = (Cell::new(0), Cell::new(0));
    let mut first = VIDEO | AUDIO;
    let second = ASYNC_VIDEO;
    assert_eq!(query(Some(first), &reads, &logs), VIDEO | AUDIO);
    assert_eq!(query(Some(second), &reads, &logs), ASYNC_VIDEO);
    assert_eq!(query(Some(first), &reads, &logs), VIDEO | AUDIO);
    first = 0x8000_0000;
    assert_eq!(query(Some(first), &reads, &logs), 0x8000_0000);
    assert_eq!(query(Some(second), &reads, &logs), ASYNC_VIDEO);
    assert_eq!((reads.get(), logs.get()), (5, 0));
}

/// NULL returns 0 with one log line and no read.
#[test]
fn null_source_returns_zero_and_logs() {
    let (reads, logs) = (Cell::new(0), Cell::new(0));
    assert_eq!(query(None, &reads, &logs), 0);
    assert_eq!((reads.get(), logs.get()), (0, 1));
}
