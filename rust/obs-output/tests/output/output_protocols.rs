//! Tier 1 mirrors the actual-library C characterization without FFI.
use obs_output::output_protocols::{Protocols, protocols};

#[test]
fn borrowed_protocols() {
    let values: [Option<&[u8]>; 4] = [None, Some(b""), Some(b"rtmp;rtmps"), Some(b"rtmp;\xff")];
    let flags = [0, 8, 1, u32::MAX, u32::MAX & !8];
    for value in values {
        for flag in flags {
            let output = Some(Protocols { flags: flag, protocols: value });
            let expected = if flag & 8 != 0 { value } else { None };
            for _ in 0..2 {
                assert_eq!(protocols(output, |_| panic!("valid output must not log")), expected);
            }
        }
    }
}

#[test]
fn null_output_logs_getter_name() {
    let mut logs = Vec::new();
    assert_eq!(protocols::<Option<&[u8]>>(None, |name| logs.push(name)), None);
    assert_eq!(logs, [c"obs_output_get_protocols"]);
}
