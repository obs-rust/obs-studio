//! Tier 3: the original C bodies' complete ordered call trace, no exclusions.
//! Callback/parameter forwarding is checked by the C recorder, including NULL.
use obs_source::audio_pause_callbacks::update;
use proptest::prelude::*;
use std::cell::Cell;

unsafe extern "C" {
    fn oracle_audio_pause_trace(
        present: bool,
        adding: bool,
        null_callback: bool,
        null_param: bool,
    ) -> u32;
}

fn check(present: bool, adding: bool, null_callback: bool, null_param: bool) {
    let code = Cell::new(0_u32);
    update(
        present,
        adding,
        || {
            code.set(code.get() * 16 + 1);
            123
        },
        |handler, signal| {
            assert_eq!(*handler, 123);
            let event = match signal.to_bytes() {
                b"media_pause" => 2,
                b"media_stopped" => 3,
                _ => panic!("unexpected signal"),
            };
            code.set(code.get() * 16 + event + if adding { 0 } else { 4 });
        },
        |_| {},
    );
    // SAFETY: the oracle owns its real-layout stack fixture and the trace;
    // no borrowed pointer escapes, and no callback is invoked.
    let original = unsafe { oracle_audio_pause_trace(present, adding, null_callback, null_param) };
    assert_eq!(code.get(), original);
}

#[test]
fn all_edges() {
    for present in [false, true] {
        for adding in [false, true] {
            for null_callback in [false, true] {
                for null_param in [false, true] {
                    check(present, adding, null_callback, null_param);
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn operation_sequences(operations in prop::collection::vec((any::<bool>(), any::<bool>(), any::<bool>(), any::<bool>()), 0..65)) {
        for (present, adding, null_callback, null_param) in operations {
            check(present, adding, null_callback, null_param);
        }
    }
}
