//! Tier 1: `obs_audio_monitoring::coreaudio_devices`, the CoreAudio
//! monitoring device rules.
//!
//! The `steps_*` cases port `test/cmocka/test_coreaudio_device_rules.c` 1:1:
//! its 11 shared cases and the one of its `COREAUDIO_DEVICE_RULES_TEST_RUST`
//! build (the port's intentional difference), under the same names, in the
//! same order, with the same values and expectations. The C test's other
//! case pins the C's count after a shorter second size, which the port
//! changes: it is `edge_c_*` here, against the model of the C below. The C
//! calls the hidden `coreaudio_device_*` functions; here the safe core is
//! called directly, with `is_virtual` for the C's `!device_is_input`.
//!
//! The `edge_*` cases are new: each function against a model of the
//! origin/master C (its expressions inline, libobs's `astrstri()` in the C
//! locale) over edge values, and proptests over random sizes, ids, names and
//! uids, non-ASCII bytes included.

use obs_audio_monitoring::coreaudio_devices::{
    VIRTUAL_NAMES, count, is_default, is_virtual, listed, names_match,
};
use proptest::prelude::*;

const ID_SIZE: usize = 4; // sizeof(AudioDeviceID)

// ---- models of the origin/master C, expressions inline ----

/// `!(!allow_inputs && !size)`
fn c_listed(allow_inputs: bool, size: u32) -> bool {
    !(!allow_inputs && size == 0)
}

/// `count = size / sizeof(AudioDeviceID)`, from the first size.
fn c_count(first_size: u32, _second_size: u32, id_size: usize) -> u32 {
    u32::try_from(first_size as usize / id_size).unwrap()
}

/// `strcmp(id, "default") == 0`
fn c_is_default(id: &[u8]) -> bool {
    id == b"default"
}

/// `name1 && name2 && strcmp(name1, name2) == 0`
fn c_names_match(name1: Option<&[u8]>, name2: Option<&[u8]>) -> bool {
    matches!((name1, name2), (Some(a), Some(b)) if a == b)
}

/// libobs's `astrstri()`: `astrcmpi_n()` with `tolower()` at every start,
/// the terminating NUL's included (do-while).
fn c_astrstri(s: &[u8], find: &[u8]) -> bool {
    let lower = |c: u8| c.to_ascii_lowercase();
    // The C string with its NUL, which ends astrcmpi_n's walk.
    let mut z = s.to_vec();
    z.push(0);
    for start in 0..z.len() {
        let mut i = 0;
        loop {
            if i == find.len() {
                return true;
            }
            let a = z.get(start + i).copied().unwrap_or(0);
            let b = find[i];
            if lower(a) != lower(b) || a == 0 {
                break;
            }
            i += 1;
        }
    }
    false
}

/// `!device_is_input(uid)`
fn c_is_virtual(uid: &[u8]) -> bool {
    !(!c_astrstri(uid, b"soundflower")
        && !c_astrstri(uid, b"wavtap")
        && !c_astrstri(uid, b"soundsiphon")
        && !c_astrstri(uid, b"ishowu")
        && !c_astrstri(uid, b"blackhole")
        && !c_astrstri(uid, b"loopback")
        && !c_astrstri(uid, b"groundcontrol")
        && !c_astrstri(uid, b"vbcable"))
}

// ---- test_coreaudio_device_rules.c, 1:1 ----

#[test]
fn steps_a_device_without_output_streams_is_listed_only_with_inputs() {
    assert!(listed(true, 0));
    assert!(!listed(false, 0));
}

#[test]
fn steps_a_device_with_output_streams_is_always_listed() {
    for size in [1, 4, 8, 0x8000_0000, u32::MAX] {
        assert!(listed(false, size));
        assert!(listed(true, size));
    }
}

#[test]
fn steps_the_count_is_whole_ids() {
    assert_eq!(count(0, 0, ID_SIZE), 0);
    assert_eq!(count(4, 4, ID_SIZE), 1);
    assert_eq!(count(12, 12, ID_SIZE), 3);
    // a partial id is not counted
    assert_eq!(count(13, 13, ID_SIZE), 3);
    assert_eq!(count(3, 3, ID_SIZE), 0);
    assert_eq!(count(u32::MAX, u32::MAX, ID_SIZE), u32::MAX / 4);
    assert_eq!(count(12, 12, 1), 12);
}

#[test]
fn steps_a_larger_second_size_does_not_grow_the_count() {
    // The array holds the first size.
    assert_eq!(count(8, 12, ID_SIZE), 2);
    assert_eq!(count(0, 4, ID_SIZE), 0);
}

#[test]
fn steps_only_default_is_the_default_id() {
    assert!(is_default(b"default"));
    for other in [
        &b"Default"[..],
        b"DEFAULT",
        b"defaulT",
        b"default ",
        b" default",
        b"defaul",
        b"defaultx",
        b"",
        b"BuiltInSpeakerDevice",
    ] {
        assert!(!is_default(other));
    }
}

#[test]
fn steps_names_match_when_both_are_found_and_equal() {
    let m = |a: &[u8], b: &[u8]| names_match(Some(a), Some(b));
    assert!(m(b"MacBook Pro Speakers", b"MacBook Pro Speakers"));
    assert!(m(b"", b""));
    assert!(m(b"\xc3\xa9", b"\xc3\xa9"));
    assert!(!m(b"MacBook Pro Speakers", b"BlackHole 2ch"));
    // case counts
    assert!(!m(b"Speakers", b"speakers"));
    assert!(!m(b"Speakers", b"Speakers "));
    assert!(!m(b"Speakers", b""));
}

#[test]
fn steps_a_name_not_found_never_matches() {
    assert!(!names_match(None, Some(b"Speakers")));
    assert!(!names_match(Some(b"Speakers"), None));
    assert!(!names_match(None, None));
    assert!(!names_match(None, Some(b"")));
    assert!(!names_match(Some(b""), None));
}

#[test]
fn steps_two_devices_with_one_name_match() {
    // devices_match() compares the names it looked up for both uids, so two
    // devices with the same name match whatever their uids.
    let name_of_uid1 = b"USB Audio";
    let name_of_uid2 = b"USB Audio";
    assert!(names_match(Some(name_of_uid1), Some(name_of_uid2)));
}

const VIRTUAL_CASES: [[&str; 3]; 8] = [
    ["soundflower", "SOUNDFLOWER", "SoundFlower"],
    ["wavtap", "WAVTAP", "WavTap"],
    ["soundsiphon", "SOUNDSIPHON", "SoundSiphon"],
    ["ishowu", "ISHOWU", "iShowU"],
    ["blackhole", "BLACKHOLE", "BlackHole"],
    ["loopback", "LOOPBACK", "LoopBack"],
    ["groundcontrol", "GROUNDCONTROL", "GroundControl"],
    ["vbcable", "VBCABLE", "VBCable"],
];

#[test]
fn steps_each_virtual_name_in_any_case_is_not_an_input() {
    for names in VIRTUAL_CASES {
        for name in names {
            assert!(is_virtual(name.as_bytes()), "{name}");
            let uid = format!("com.example.{name}:2ch_UID");
            assert!(is_virtual(uid.as_bytes()), "{uid}");
            let uid = format!("{name}_UID");
            assert!(is_virtual(uid.as_bytes()), "{uid}");
            let mut uid = b"\xc3\xa9".to_vec();
            uid.extend_from_slice(name.as_bytes());
            uid.extend_from_slice(b"\xc3\xa9");
            assert!(is_virtual(&uid), "{uid:?}");
        }
    }
}

#[test]
fn steps_a_uid_without_a_virtual_name_is_an_input() {
    for uid in [
        &b"BuiltInSpeakerDevice"[..],
        b"AppleUSBAudioEngine:Generic:USB Audio:14200000:2",
        b"",
        // near misses
        b"blackhol",
        b"black hole",
        b"Black-Hole",
        b"VB-Cable",
        b"sound flower",
        b"loopbac",
        b"ishow",
        // only ASCII case is folded: a fullwidth B and a dotted I
        b"\xef\xbc\xa2lackHole",
        b"\xc4\xb0ShowU",
    ] {
        assert!(!is_virtual(uid), "{uid:?}");
    }
}

#[test]
fn steps_a_null_uid_is_an_input() {
    // The C's NULL uid is the shim's; the core takes bytes. astrstri()
    // finds nothing in NULL, as in an empty uid.
    assert!(!is_virtual(b""));
}

#[test]
fn steps_the_port_counts_only_the_ids_written() {
    assert_eq!(count(12, 8, ID_SIZE), 2);
    assert_eq!(count(12, 0, ID_SIZE), 0);
    assert_eq!(count(13, 9, ID_SIZE), 2);
    assert_eq!(count(u32::MAX, 4, ID_SIZE), 1);
}

// ---- the C's case the port changes, against the model ----

#[test]
fn edge_c_counts_the_first_size() {
    assert_eq!(c_count(12, 8, ID_SIZE), 3);
    assert_eq!(c_count(12, 0, ID_SIZE), 3);
    assert_eq!(c_count(13, 9, ID_SIZE), 3);
    assert_eq!(c_count(u32::MAX, 4, ID_SIZE), u32::MAX / 4);
}

// ---- edge values against the model ----

const SIZES: [u32; 14] = [
    0,
    1,
    3,
    4,
    5,
    7,
    8,
    12,
    13,
    16,
    0x7fff_ffff,
    0x8000_0000,
    u32::MAX - 1,
    u32::MAX,
];

#[test]
fn edge_listed() {
    for allow in [false, true] {
        for size in SIZES {
            assert_eq!(listed(allow, size), c_listed(allow, size), "{allow} {size}");
        }
    }
}

#[test]
fn edge_count_matches_the_c_unless_the_second_size_is_smaller() {
    for first in SIZES {
        for second in SIZES {
            for id_size in [1, 2, 4, 8, 3, usize::MAX] {
                let got = count(first, second, id_size);
                let want = c_count(first.min(second), first.min(second), id_size);
                assert_eq!(got, want, "{first} {second} {id_size}");
                if second >= first {
                    assert_eq!(got, c_count(first, second, id_size));
                }
            }
        }
    }
}

#[test]
fn edge_count_with_no_id_size_is_zero() {
    // The C divides by zero; the caller passes sizeof(AudioDeviceID).
    for first in SIZES {
        assert_eq!(count(first, first, 0), 0);
    }
}

#[test]
fn edge_is_default() {
    for id in [
        &b"default"[..],
        b"default\x00",
        b"\x00default",
        b"defaul",
        b"Default",
        b"",
        b"d",
        b"defaultdefault",
    ] {
        assert_eq!(is_default(id), c_is_default(id), "{id:?}");
    }
}

#[test]
fn edge_names_match() {
    let names: [Option<&[u8]>; 6] = [
        None,
        Some(b""),
        Some(b"a"),
        Some(b"A"),
        Some(b"a "),
        Some(b"\xff"),
    ];
    for a in names {
        for b in names {
            assert_eq!(names_match(a, b), c_names_match(a, b), "{a:?} {b:?}");
        }
    }
}

#[test]
fn edge_virtual_names_are_the_c_list() {
    assert_eq!(
        VIRTUAL_NAMES,
        [
            "soundflower",
            "wavtap",
            "soundsiphon",
            "ishowu",
            "blackhole",
            "loopback",
            "groundcontrol",
            "vbcable"
        ]
    );
}

#[test]
fn edge_is_virtual_at_every_position_and_every_byte() {
    // Each name with each byte in place of each of its letters, at the
    // start, the end and in the middle of a uid.
    for name in VIRTUAL_NAMES {
        for at in 0..name.len() {
            for byte in 0..=255u8 {
                if byte == 0 {
                    continue;
                }
                let mut n = name.as_bytes().to_vec();
                n[at] = byte;
                for (pre, post) in [
                    (&b""[..], &b""[..]),
                    (b"x", b""),
                    (b"", b"y"),
                    (b"\xc3", b"\xa9"),
                ] {
                    let mut uid = pre.to_vec();
                    uid.extend_from_slice(&n);
                    uid.extend_from_slice(post);
                    assert_eq!(is_virtual(&uid), c_is_virtual(&uid), "{uid:?}");
                }
            }
        }
    }
}

#[test]
fn edge_is_virtual_overlapping_and_cut_names() {
    for uid in [
        &b"blackblackhole"[..],
        b"loooopback",
        b"loopbackloopback",
        b"soundsiphonflower",
        b"soundflowe",
        b"oundflower",
        b"vbcabl",
        b"VbCaBlE",
        b"GROUNDcontrol",
        b"isHOWu",
        b"wavta",
        b"WAVTAPP",
        b"\x80\xff\xfe",
    ] {
        assert_eq!(is_virtual(uid), c_is_virtual(uid), "{uid:?}");
    }
}

// ---- proptests ----

/// A uid of random bytes, sometimes around a virtual name in random case.
fn uid() -> impl Strategy<Value = Vec<u8>> {
    let noise = prop::collection::vec(1..=255u8, 0..12);
    (
        noise.clone(),
        noise,
        prop::option::of((0..VIRTUAL_NAMES.len(), any::<u16>(), 0..14usize)),
    )
        .prop_map(|(mut pre, post, name)| {
            if let Some((i, case_bits, cut)) = name {
                for (k, c) in VIRTUAL_NAMES[i].bytes().enumerate().take(cut.max(1)) {
                    pre.push(if case_bits >> (k % 16) & 1 == 1 {
                        c.to_ascii_uppercase()
                    } else {
                        c
                    });
                }
            }
            pre.extend_from_slice(&post);
            pre
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]

    #[test]
    fn prop_listed(allow in any::<bool>(), size in any::<u32>()) {
        prop_assert_eq!(listed(allow, size), c_listed(allow, size));
    }

    #[test]
    fn prop_count_is_the_smaller_size(first in any::<u32>(), second in any::<u32>(), id_size in 1..=16usize) {
        let got = count(first, second, id_size);
        prop_assert_eq!(got, c_count(first.min(second), 0, id_size));
        // never more ids than either size holds
        prop_assert!(got as usize * id_size <= first.min(second) as usize);
    }

    #[test]
    fn prop_is_default(id in prop::collection::vec(any::<u8>(), 0..10)) {
        prop_assert_eq!(is_default(&id), c_is_default(&id));
    }

    #[test]
    fn prop_names_match(
        a in prop::option::of(prop::collection::vec(1..=255u8, 0..4)),
        b in prop::option::of(prop::collection::vec(1..=255u8, 0..4)),
    ) {
        prop_assert_eq!(names_match(a.as_deref(), b.as_deref()), c_names_match(a.as_deref(), b.as_deref()));
    }

    #[test]
    fn prop_is_virtual(uid in uid()) {
        prop_assert_eq!(is_virtual(&uid), c_is_virtual(&uid));
    }
}
