//! Tier 1 mirrors test_source_interaction.c using safe borrowed payloads.
use obs_source::interaction::{INTERACTION, dispatch};

const NAMES: [&core::ffi::CStr; 5] = [
    c"obs_source_send_mouse_click",
    c"obs_source_send_mouse_move",
    c"obs_source_send_mouse_wheel",
    c"obs_source_send_focus",
    c"obs_source_send_key_click",
];

#[test]
fn test_payload_forwarding() {
    let data = 19_u8;
    let mouse = (u32::MAX, i32::MIN, i32::MAX);
    let key = (
        u32::MAX,
        "key payload",
        0x8765_4321_u32,
        0x1234_5678_u32,
        u32::MAX,
    );
    for name in NAMES {
        let mut calls = 0;
        for pass in 0..3 {
            let payload = (
                &data,
                (pass != 2).then_some(&mouse),
                (pass != 2).then_some(&key),
                pass != 0,
                if pass == 0 { 2 } else { i32::MIN },
                if pass == 0 { 0 } else { u32::MAX },
                if pass == 0 { i32::MIN } else { i32::MAX },
                if pass == 0 { i32::MAX } else { i32::MIN },
            );
            let callback = |actual| {
                assert_eq!(payload, actual);
                assert!(core::ptr::eq(payload.0, actual.0));
                if let Some(event) = actual.1 {
                    assert!(core::ptr::eq(event, &mouse));
                }
                if let Some(event) = actual.2 {
                    assert!(core::ptr::eq(event, &key));
                }
                calls += 1;
            };
            dispatch(Some((INTERACTION, Some(callback))), payload, name, |_| {
                panic!("valid source")
            });
            assert_eq!(calls, pass + 1);
        }
    }
}

#[test]
fn test_gating_and_optional_callbacks() {
    for name in NAMES {
        for flags in [0, !INTERACTION] {
            dispatch(
                Some((flags, Some(|()| panic!("disabled")))),
                (),
                name,
                |_| panic!("valid source"),
            );
        }
        dispatch::<_, fn(())>(Some((INTERACTION, None)), (), name, |_| {
            panic!("valid source")
        });
    }
}

#[test]
fn test_null_source() {
    for name in NAMES {
        let mut logs = Vec::new();
        dispatch::<_, fn(())>(None, (), name, |actual| logs.push(actual));
        assert_eq!(logs, [name]);
    }
}
