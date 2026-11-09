//! Tier 1 mirrors test_graphics_shader_params.c without FFI or unsafe code.
use obs_graphics::shader_params::{WriteError, write};

#[test]
fn validation_mirrors_c_validation() {
    for kind in 0..8 {
        for context in [false, true] {
            for parameter in [None, Some(19)] {
                for present in [false, true] {
                    let value = (present || kind < 3).then_some(23);
                    let mut calls = 0;
                    let result = write(context, parameter, value, |p, v| {
                        calls += 1;
                        assert_eq!((p, v), (19, 23));
                    });
                    let expected = if !context {
                        Err(WriteError::NoContext)
                    } else if parameter.is_none() {
                        Err(WriteError::NullParameter)
                    } else if value.is_none() {
                        Err(WriteError::NullValue)
                    } else {
                        Ok(())
                    };
                    assert_eq!(result, expected);
                    assert_eq!(calls, usize::from(expected.is_ok()));
                }
            }
        }
    }
    assert_eq!(
        WriteError::NoContext.reason(),
        "called while not in a graphics context"
    );
    assert_eq!(WriteError::NullParameter.reason(), "Null 'param' parameter");
    assert_eq!(WriteError::NullValue.reason(), "Null 'val' parameter");
}

fn forwarded<V: Copy>(value: V, check: impl FnOnce(V)) {
    let mut calls = 0;
    assert_eq!(
        write(true, Some(19), Some(value), |p, v| {
            calls += 1;
            assert_eq!(p, 19);
            check(v);
        }),
        Ok(())
    );
    assert_eq!(calls, 1);
}

#[test]
fn writes_mirrors_c_scalar_writes() {
    for value in [false, true] {
        forwarded(value, |seen| assert_eq!(seen, value));
    }
    for value in [
        0.0_f32,
        -0.0,
        1.25,
        -42.5,
        f32::MAX,
        f32::MIN_POSITIVE,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
    ] {
        forwarded(value, |seen| assert_eq!(seen.to_bits(), value.to_bits()));
    }
    for value in [0, 1, -1, i32::MIN, i32::MAX, 1_234_567] {
        forwarded(value, |seen| assert_eq!(seen, value));
    }
}

#[test]
fn writes_mirrors_c_pointer_writes() {
    for (kind, size) in [(3, 48), (4, 64), (5, 8), (6, 16), (7, 16)] {
        let value = vec![kind + 1; size];
        let snapshot = value.clone();
        for input in [&value, &snapshot] {
            forwarded(input, |seen| assert!(std::ptr::eq(seen, input)));
            assert_eq!(value, snapshot);
        }
    }
}
