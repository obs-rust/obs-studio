use obs_output::output_mixer_setters::{Decision, decide};
#[test]
fn characterized_setters() {
    assert_eq!(
        decide(false, false, None, || panic!("null active read"), 0),
        Decision::Null
    );
    for encoded in [false, true] {
        for active in [false, true] {
            for value in [0, 1, 2, 3, 4, 5, 6, 63, 64, usize::MAX] {
                for index in [None, Some(value)] {
                    let expected = if encoded {
                        Decision::Encoded
                    } else if index.is_some_and(|i| i >= 6) {
                        Decision::InvalidIndex
                    } else if active {
                        Decision::Active
                    } else {
                        Decision::Set(index.map_or(value, |i| 1 << i))
                    };
                    assert_eq!(decide(true, encoded, index, || active, value), expected);
                }
            }
        }
    }
}
#[test]
fn gate_order() {
    assert_eq!(
        decide(
            true,
            true,
            Some(usize::MAX),
            || panic!("encoded active read"),
            0
        ),
        Decision::Encoded
    );
    assert_eq!(
        decide(true, false, Some(6), || panic!("invalid active read"), 0),
        Decision::InvalidIndex
    );
}
