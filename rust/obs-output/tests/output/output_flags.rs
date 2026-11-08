//! Tier 1 mirrors test_output_flags.c, including every flag bit and NULL.
use obs_output::output_flags::flags;

#[test]
fn flags_match_characterization() {
    for value in [0, 1, 3, 7, 0x8000_0000, 0xaaaa_aaaa, u32::MAX]
        .into_iter()
        .chain((0..32).map(|bit| 1u32 << bit))
    {
        let before = value;
        for _ in 0..2 {
            assert_eq!(
                flags(Some(value), |_| panic!("valid output must not log")),
                value
            );
            assert_eq!(value, before);
        }
    }
}

#[test]
fn null_output() {
    let mut logs = Vec::new();
    assert_eq!(flags(None, |name| logs.push(name)), 0);
    assert_eq!(logs, [c"obs_output_get_flags"]);
}
