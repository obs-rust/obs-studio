//! Tier 1: the legacy preferred-size wrapper's gates, mirroring
//! test/cmocka/test_output_preferred_size.c.
use core::cell::Cell;
use obs_output::output_preferred_size::{DEFAULT_SLOT, Decision, decide};

const VIDEO: u32 = 1 << 0;
const AUDIO: u32 = 1 << 1;
const ENCODED: u32 = 1 << 2;
const SERVICE: u32 = 1 << 3;

#[test]
fn null_output_never_reads_flags() {
    assert_eq!(
        decide(false, || panic!("flags read for NULL")),
        Decision::Null
    );
}

#[test]
fn non_video_outputs_stop_before_delegation() {
    for flags in [
        0,
        AUDIO,
        ENCODED,
        SERVICE,
        AUDIO | ENCODED | SERVICE,
        !VIDEO,
    ] {
        assert_eq!(
            decide(true, || flags),
            Decision::NotVideo,
            "flags={flags:#x}"
        );
    }
}

#[test]
fn video_outputs_delegate_to_slot_zero() {
    assert_eq!(DEFAULT_SLOT, 0);
    for flags in [
        VIDEO,
        VIDEO | AUDIO,
        VIDEO | ENCODED,
        VIDEO | AUDIO | ENCODED | SERVICE,
        u32::MAX,
    ] {
        assert_eq!(
            decide(true, || flags),
            Decision::Delegate,
            "flags={flags:#x}"
        );
    }
}

#[test]
fn flags_are_read_once() {
    for flags in [0, VIDEO] {
        let reads = Cell::new(0);
        decide(true, || {
            reads.set(reads.get() + 1);
            flags
        });
        assert_eq!(reads.get(), 1);
    }
}
