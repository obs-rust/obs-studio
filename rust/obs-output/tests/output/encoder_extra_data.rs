//! Tier 1 mirrors test_encoder_extra_data.c using safe callback-owned outputs.
use obs_output::encoder_extra_data::get_extra_data;
use std::cell::Cell;

#[test]
fn null_encoder() {
    let token = 7u8;
    let data = Some(&token);
    let size = usize::MAX;
    let logs = Cell::new(0);
    assert!(!get_extra_data::<fn() -> bool>(None, || logs.set(logs.get() + 1)));
    assert_eq!(data, Some(&token));
    assert_eq!(size, usize::MAX);
    assert_eq!(logs.get(), 1);
}

#[test]
fn optional_gate() {
    for callback in [false, true] {
        for context in [false, true] {
            if callback && context {
                continue;
            }
            let calls = Cell::new(0);
            let run = || {
                calls.set(calls.get() + 1);
                true
            };
            let token = 7u8;
            let data = Some(&token);
            let size = usize::MAX;
            for _ in 0..2 {
                assert!(!get_extra_data(
                    Some((context, callback.then_some(run))),
                    || panic!("live encoder")
                ));
            }
            assert_eq!(calls.get(), 0);
            assert_eq!(data, Some(&token));
            assert_eq!(size, usize::MAX);
        }
    }
}

#[test]
fn dispatch_and_outputs() {
    let sentinel = 0u8;
    let payload = 1u8;
    for result in [false, true] {
        for mutate in [false, true] {
            for null_data in [false, true] {
                for value in [
                    0,
                    1,
                    usize::MAX / 2,
                    usize::MAX / 2 + 1,
                    usize::MAX - 1,
                    usize::MAX,
                ] {
                    let mut data = Some(&sentinel);
                    let mut size = 17;
                    let calls = Cell::new(0);
                    let output = if null_data { None } else { Some(&payload) };
                    let context = &calls;
                    let callback = || {
                        assert!(std::ptr::eq(context, &calls));
                        context.set(context.get() + 1);
                        if mutate {
                            data = output;
                            size = value;
                        }
                        result
                    };
                    assert_eq!(
                        get_extra_data(Some((true, Some(callback))), || panic!("live encoder")),
                        result
                    );
                    assert_eq!(calls.get(), 1);
                    assert_eq!(data, if mutate { output } else { Some(&sentinel) });
                    assert_eq!(size, if mutate { value } else { 17 });
                }
            }
        }
    }
}

#[test]
fn nullable_outputs() {
    let sentinel = 0u8;
    let payload = 1u8;
    for result in [false, true] {
        for outputs in 0..4 {
            let mut data = Some(&sentinel);
            let mut size = 17;
            let calls = Cell::new(0);
            let data_output = (outputs & 1 != 0).then_some(&mut data);
            let size_output = (outputs & 2 != 0).then_some(&mut size);
            let callback = || {
                calls.set(calls.get() + 1);
                if let Some(data) = data_output {
                    *data = Some(&payload);
                }
                if let Some(size) = size_output {
                    *size = usize::MAX;
                }
                result
            };
            assert_eq!(
                get_extra_data(Some((true, Some(callback))), || panic!("live encoder")),
                result
            );
            assert_eq!(calls.get(), 1);
            assert_eq!(
                data,
                if outputs & 1 != 0 {
                    Some(&payload)
                } else {
                    Some(&sentinel)
                }
            );
            assert_eq!(size, if outputs & 2 != 0 { usize::MAX } else { 17 });
        }
    }
}
