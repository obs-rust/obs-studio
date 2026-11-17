//! Original C callbacks and Rust must observe the same ordering and field values.
use super::push_hotkeys::World;
use obs_source::push_hotkeys::{Kind, event};
use proptest::prelude::*;
unsafe extern "C" {
    fn oracle_push_hotkey_event(
        timestamp: u64,
        mute: bool,
        talk: bool,
        kind: bool,
        pressed: bool,
        out: *mut u64,
    );
}
fn compare(events: &[(u64, bool, bool)], mute: bool, talk: bool) {
    let mut world = World {
        mute,
        talk,
        ..World::default()
    };
    let mut expected = Vec::new();
    for &(now, is_talk, pressed) in events {
        let mut c = [0; 11];
        // SAFETY: the oracle writes eleven u64 values into this array, without retaining it.
        unsafe {
            oracle_push_hotkey_event(
                now,
                world.mute,
                world.talk,
                is_talk,
                pressed,
                c.as_mut_ptr(),
            )
        };
        world.now = now;
        world.trace.clear();
        let kind = if is_talk { Kind::Talk } else { Kind::Mute };
        event(&mut world, kind, pressed);
        expected.push((
            c[0],
            if c[1] == 2 {
                Kind::Talk
            } else {
                assert_eq!(c[1], 3);
                Kind::Mute
            },
            c[2] != 0,
        ));
        assert_eq!(world.actions, expected);
        assert_eq!((world.mute, world.talk), (c[3] != 0, c[4] != 0));
        assert_eq!(world.trace, c[5..]);
        assert!(!world.locked);
    }
}
#[test]
fn edges_and_all_short_sequences() {
    for mute in [false, true] {
        for talk in [false, true] {
            for bits in 0..256_u32 {
                let events: Vec<_> = (0..4)
                    .map(|i| {
                        (
                            [0, 1, u64::MAX, 0][i],
                            bits & (1 << (i * 2)) != 0,
                            bits & (2 << (i * 2)) != 0,
                        )
                    })
                    .collect();
                compare(&events, mute, talk);
            }
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(128)))]
    #[test]
    fn arbitrary_event_sequences(events in prop::collection::vec((any::<u64>(), any::<bool>(), any::<bool>()), 0..64), mute in any::<bool>(), talk in any::<bool>()) {
        compare(&events, mute, talk);
    }
}
