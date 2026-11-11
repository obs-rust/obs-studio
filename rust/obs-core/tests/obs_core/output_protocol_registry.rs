use obs_core::output_protocol_registry::{enumerate, registered};

const PROTOCOLS: [&[u8]; 8] = [
    b"RTMP", b"RTMPS", b"SRT", b"RTMPX", b"rtmp", b"HLS", b"WHIP", b" RIST",
];

#[test]
fn test_output_protocols_registered_protocol_list() {
    for (index, protocol) in PROTOCOLS.iter().enumerate() {
        assert_eq!(enumerate(&PROTOCOLS, index), Some(*protocol));
    }
    assert_eq!(enumerate(&PROTOCOLS, PROTOCOLS.len()), None);
    assert_eq!(enumerate(&PROTOCOLS, usize::MAX), None);
    assert_eq!(enumerate::<&[u8]>(&[], 0), None);
}

#[test]
fn test_output_protocols_is_registered() {
    for protocol in PROTOCOLS {
        assert!(registered(Some(protocol), PROTOCOLS));
    }
    for protocol in [
        "",
        "Rtmp",
        "RTM",
        "RTMP;RTMPS",
        "RTMP;",
        ";",
        "HLS;",
        "RIST",
        " ",
        "PLAINONLY",
        "FOO",
        "srt",
        "RTMP ",
        "rtmps",
    ] {
        assert!(!registered(Some(protocol.as_bytes()), PROTOCOLS));
    }
}

#[test]
fn test_output_protocol_null_registered_protocol_still_found() {
    let protocols = [b"TESTPROTO".as_slice()];
    assert!(registered(Some(b"TESTPROTO"), protocols));
    assert!(!registered(Some(b"NOPROTO"), protocols));
    assert!(!registered(None, protocols));
    assert_eq!(enumerate(&protocols, 0), Some(b"TESTPROTO".as_slice()));
}

#[test]
fn duplicates_empty_and_non_utf8_remain_exact() {
    let protocols = [b"".as_slice(), b"\xff", b"RTMP", b"RTMP"];
    assert!(registered(Some(b""), protocols));
    assert!(registered(Some(b"\xff"), protocols));
    assert!(!registered(Some(b"\xfe"), protocols));
    assert!(!registered(Some(b""), []));
    assert_eq!(enumerate(&protocols, 2), enumerate(&protocols, 3));
}
