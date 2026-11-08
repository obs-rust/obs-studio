//! Tier 1 mirrors every assertion in test_output_mixer_getters.c.
use obs_output::output_mixer_getters::{mixer, mixers};

#[test]
fn masks() {
    let special = [0, 1, 2, 3, 32, 64, 65, 128, usize::MAX];
    for mask in special
        .into_iter()
        .chain((0..usize::BITS).map(|bit| 1usize << bit))
    {
        let expected = (0..6).find(|bit| mask & (1usize << bit) != 0).unwrap_or(0);
        for _ in 0..2 {
            assert_eq!(
                mixer(Some(mask), |_| panic!("valid output must not log")),
                expected
            );
            assert_eq!(
                mixers(Some(mask), |_| panic!("valid output must not log")),
                mask
            );
        }
    }
}

#[test]
fn null_output() {
    let mut logs = Vec::new();
    assert_eq!(mixer(None, |name| logs.push(name)), 0);
    assert_eq!(mixers(None, |name| logs.push(name)), 0);
    assert_eq!(logs, [c"obs_output_get_mixer", c"obs_output_get_mixers"]);
}
