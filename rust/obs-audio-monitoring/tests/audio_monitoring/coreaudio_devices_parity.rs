//! Tier 3: the `coreaudio_device_*` shims and the safe core against the
//! original C (`obs_c_oracle::coreaudio_device_rules`, the unmodified
//! `libobs/audio-monitoring/osx/coreaudio-device-rules.c.h` with libobs's C
//! `astrstri()`).
//!
//! Every function is compared over edge values and random inputs: oracle,
//! shim and core give the same answer, the strings passed as C strings
//! (random bytes 1 to 255, so bytes above 127 too) and NULL where the C
//! accepts one.
//!
//! Intentional difference (#3359), excluded exactly: `count` when the
//! second size is smaller than the first. The C keeps the first size, so
//! `enum_audio_devices()` walks ids the second call never wrote; the port
//! takes the smaller size. For those inputs the port is compared with the
//! oracle given the smaller size twice, and everywhere else with the oracle
//! as is.
//!
//! Inputs the C leaves undefined are left out: a zero id size (the C
//! divides by it) and a NULL id for `is_default` (the C dereferences it).
//! The shims count none and answer no there.

use std::ffi::{CString, c_char};
use std::ptr;

use obs_audio_monitoring::coreaudio_devices::{
    VIRTUAL_NAMES, count, is_default, is_virtual, listed, names_match,
};
use obs_audio_monitoring::ffi::coreaudio_devices::{
    coreaudio_device_count, coreaudio_device_is_default, coreaudio_device_is_input,
    coreaudio_device_listed, coreaudio_device_names_match,
};
use obs_c_oracle::coreaudio_device_rules::{
    oracle_coreaudio_device_count, oracle_coreaudio_device_is_default,
    oracle_coreaudio_device_is_input, oracle_coreaudio_device_listed,
    oracle_coreaudio_device_names_match,
};
use proptest::prelude::*;

const ID_SIZE: usize = 4; // sizeof(AudioDeviceID)

fn ptr_of(s: Option<&CString>) -> *const c_char {
    s.map_or(ptr::null(), |s| s.as_ptr())
}

fn c(bytes: &[u8]) -> CString {
    CString::new(bytes).expect("no NUL")
}

fn check_listed(allow: bool, size: u32) {
    // SAFETY: plain values.
    let want = unsafe { oracle_coreaudio_device_listed(allow, size) };
    assert_eq!(coreaudio_device_listed(allow, size), want, "{allow} {size}");
    assert_eq!(listed(allow, size), want, "{allow} {size}");
}

fn check_count(first: u32, second: u32, id_size: usize) {
    // The intentional difference: the C given the smaller size as the first.
    let first_for_c = if second < first { second } else { first };
    // SAFETY: plain values, id_size nonzero.
    let want = unsafe { oracle_coreaudio_device_count(first_for_c, second, id_size) };
    assert_eq!(
        coreaudio_device_count(first, second, id_size),
        want,
        "{first} {second} {id_size}"
    );
    assert_eq!(
        count(first, second, id_size),
        want,
        "{first} {second} {id_size}"
    );
    if second >= first {
        // SAFETY: as above.
        let c_as_is = unsafe { oracle_coreaudio_device_count(first, second, id_size) };
        assert_eq!(count(first, second, id_size), c_as_is);
    }
}

fn check_is_default(id: &[u8]) {
    let s = c(id);
    // SAFETY: a NUL-terminated string.
    let want = unsafe { oracle_coreaudio_device_is_default(s.as_ptr()) };
    // SAFETY: as above.
    assert_eq!(
        unsafe { coreaudio_device_is_default(s.as_ptr()) },
        want,
        "{id:?}"
    );
    assert_eq!(is_default(id), want, "{id:?}");
}

fn check_names_match(a: Option<&[u8]>, b: Option<&[u8]>) {
    let (ca, cb) = (a.map(c), b.map(c));
    let (pa, pb) = (ptr_of(ca.as_ref()), ptr_of(cb.as_ref()));
    // SAFETY: each null or a NUL-terminated string.
    let want = unsafe { oracle_coreaudio_device_names_match(pa, pb) };
    // SAFETY: as above.
    assert_eq!(
        unsafe { coreaudio_device_names_match(pa, pb) },
        want,
        "{a:?} {b:?}"
    );
    assert_eq!(names_match(a, b), want, "{a:?} {b:?}");
}

fn check_is_input(uid: Option<&[u8]>) {
    let s = uid.map(c);
    let p = ptr_of(s.as_ref());
    // SAFETY: null or a NUL-terminated string; astrstri() takes NULL.
    let want = unsafe { oracle_coreaudio_device_is_input(p) };
    // SAFETY: as above.
    assert_eq!(unsafe { coreaudio_device_is_input(p) }, want, "{uid:?}");
    if let Some(uid) = uid {
        assert_eq!(!is_virtual(uid), want, "{uid:?}");
    }
}

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
fn parity_listed_every_edge() {
    for allow in [false, true] {
        for size in SIZES {
            check_listed(allow, size);
        }
    }
}

#[test]
fn parity_count_every_edge_pair() {
    for first in SIZES {
        for second in SIZES {
            for id_size in [1, 2, 3, 4, 8, 16, usize::MAX] {
                check_count(first, second, id_size);
            }
        }
    }
}

#[test]
fn parity_count_differs_only_when_the_second_size_is_smaller() {
    // The listed difference, shown on the oracle: it keeps the first size.
    // SAFETY: plain values.
    assert_eq!(unsafe { oracle_coreaudio_device_count(12, 8, ID_SIZE) }, 3);
    assert_eq!(count(12, 8, ID_SIZE), 2);
    // SAFETY: plain values.
    assert_eq!(
        unsafe { oracle_coreaudio_device_count(8, 12, ID_SIZE) },
        count(8, 12, ID_SIZE)
    );
}

#[test]
fn parity_is_default_edges() {
    for id in [
        &b"default"[..],
        b"Default",
        b"DEFAULT",
        b"defaul",
        b"defaultx",
        b" default",
        b"",
        b"\xffdefault",
    ] {
        check_is_default(id);
    }
}

#[test]
fn parity_names_match_edges() {
    let names: [Option<&[u8]>; 7] = [
        None,
        Some(b""),
        Some(b"Speakers"),
        Some(b"speakers"),
        Some(b"Speakers "),
        Some(b"\xc3\xa9"),
        Some(b"\xc3\x89"),
    ];
    for a in names {
        for b in names {
            check_names_match(a, b);
        }
    }
}

#[test]
fn parity_is_input_every_byte_in_every_name() {
    check_is_input(None);
    check_is_input(Some(b""));
    for name in VIRTUAL_NAMES {
        check_is_input(Some(name.as_bytes()));
        check_is_input(Some(name.to_ascii_uppercase().as_bytes()));
        for at in 0..name.len() {
            for byte in 1..=255u8 {
                let mut n = name.as_bytes().to_vec();
                n[at] = byte;
                check_is_input(Some(&n));
                let mut uid = b"\xc3".to_vec();
                uid.extend_from_slice(&n);
                uid.push(b'x');
                check_is_input(Some(&uid));
            }
            // cut short at each length
            check_is_input(Some(&name.as_bytes()[..at]));
        }
    }
}

/// A uid of random bytes, sometimes around a virtual name in random case,
/// cut short at random.
fn uid() -> impl Strategy<Value = Vec<u8>> {
    let noise = prop::collection::vec(1..=255u8, 0..12);
    (
        noise.clone(),
        noise,
        prop::option::of((0..VIRTUAL_NAMES.len(), any::<u16>(), 0..14usize)),
    )
        .prop_map(|(mut pre, post, name)| {
            if let Some((i, case_bits, cut)) = name {
                for (k, ch) in VIRTUAL_NAMES[i].bytes().enumerate().take(cut.max(1)) {
                    pre.push(if case_bits >> (k % 16) & 1 == 1 {
                        ch.to_ascii_uppercase()
                    } else {
                        ch
                    });
                }
            }
            pre.extend_from_slice(&post);
            pre
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(1024)))]

    #[test]
    fn parity_listed(allow in any::<bool>(), size in any::<u32>()) {
        check_listed(allow, size);
    }

    #[test]
    fn parity_count(first in any::<u32>(), second in any::<u32>(), id_size in 1..=64usize) {
        check_count(first, second, id_size);
    }

    #[test]
    fn parity_count_near(first in 0..64u32, second in 0..64u32) {
        check_count(first, second, ID_SIZE);
    }

    #[test]
    fn parity_is_default(id in prop::collection::vec(1..=255u8, 0..9)) {
        check_is_default(&id);
    }

    #[test]
    fn parity_names_match(
        a in prop::option::of(prop::collection::vec(1..=255u8, 0..4)),
        b in prop::option::of(prop::collection::vec(1..=255u8, 0..4)),
    ) {
        check_names_match(a.as_deref(), b.as_deref());
    }

    #[test]
    fn parity_is_input(uid in prop::option::of(uid())) {
        check_is_input(uid.as_deref());
    }
}
