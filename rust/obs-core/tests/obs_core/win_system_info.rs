//! Tier 1: `obs_core::win_system_info`, the Windows system info log
//! rules.
//!
//! The `steps_*` cases port `test/cmocka/test_windows_system_info_steps.c`
//! 1:1: its 10 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones included.
//! The C calls the hidden `obs_windows_system_info_*` functions and puts
//! each line together as `obs-windows.c` does (the value only when the read
//! found it, the build only for a feature with a fallback); here the safe
//! core is called directly, and the line is put together the same way.
//!
//! The `edge_*` cases are new: proptests over any version, size, status,
//! value, build, feature, state and type, against models written from the
//! C's expressions.

use std::ffi::CStr;

use obs_core::win_system_info::{
    ERROR_SUCCESS, FEATURES, Feature, GAMING_MIN_VERSION, Line, PROVIDER_ANTISPYWARE,
    PROVIDER_ANTIVIRUS, PROVIDER_FIREWALL, STATE_EXPIRED, STATE_OFF, STATE_ON, STATE_SNOOZED,
    UNKNOWN, fallback_line, game_mode_retry, gaming_found, gaming_logged, has_fallback, mb,
    product_state, product_type, value_line, version,
};
use proptest::prelude::*;

const ERROR_FILE_NOT_FOUND: i32 = 2;
const ERROR_ACCESS_DENIED: i32 = 5;

fn text(s: &CStr) -> &str {
    s.to_str().expect("the labels are ASCII")
}

/// One feature's line, put together as `log_gaming_features()` does.
fn gaming_line(feature: Feature, status: i32, value: u32, win_build: u32) -> Option<Line> {
    if gaming_found(status) {
        Some(value_line(feature, value))
    } else if has_fallback(feature) {
        fallback_line(feature, win_build)
    } else {
        None
    }
}

/// One feature's line, as `obs-windows.c` logs it, or `None`.
fn line_of(index: i32, status: i32, value: u32, win_build: u32) -> Option<String> {
    let feature = Feature::from_index(index)?;
    gaming_line(feature, status, value, win_build)
        .map(|line| format!("\t{}: {}", text(line.label), text(line.text)))
}

#[test]
fn steps_header_values() {
    assert_eq!(ERROR_SUCCESS, 0);
    assert_eq!(STATE_ON, 0);
    assert_eq!(STATE_OFF, 1);
    assert_eq!(STATE_SNOOZED, 2);
    assert_eq!(STATE_EXPIRED, 3);
    assert_eq!(PROVIDER_FIREWALL, 1);
    assert_eq!(PROVIDER_ANTIVIRUS, 4);
    assert_eq!(PROVIDER_ANTISPYWARE, 8);
    assert_eq!(Feature::GameBar as i32, 0);
    assert_eq!(Feature::GameDvrAllowed as i32, 1);
    assert_eq!(Feature::GameDvr as i32, 2);
    assert_eq!(Feature::GameDvrBgRecording as i32, 3);
    assert_eq!(Feature::GameMode as i32, 4);
    assert_eq!(Feature::ALL.len(), 5);
    assert_eq!(FEATURES.len(), 5);
}

#[test]
fn steps_version_packing() {
    assert_eq!(version(6, 1), 0x601);
    assert_eq!(version(6, 3), 0x603);
    assert_eq!(version(10, 0), 0xA00);
    assert_eq!(version(0, 0), 0);

    // characterized, not endorsed: a minor of 256 or more runs into the
    // major (Windows never reports one)
    assert_eq!(version(10, 0x100), 0xB00);
    assert_eq!(version(6, 0x1FF), 0x7FF);

    // the major's high bits shift out of 32 bits
    assert_eq!(version(0x0100_0000, 1), 1);
    assert_eq!(version(u32::MAX, 0), 0xFFFF_FF00);
}

#[test]
fn steps_gaming_gate() {
    // Windows 11 reports 10.0 too
    assert!(gaming_logged(0xA00));
    assert!(gaming_logged(0xA01));
    assert!(gaming_logged(u32::MAX));
    assert!(!gaming_logged(0x9FF));
    assert!(!gaming_logged(0x603));
    assert!(!gaming_logged(0x601));
    assert!(!gaming_logged(0));
}

#[test]
fn steps_game_mode_retry() {
    assert!(!game_mode_retry(ERROR_SUCCESS));
    assert!(game_mode_retry(ERROR_FILE_NOT_FOUND));
    assert!(game_mode_retry(ERROR_ACCESS_DENIED));
    assert!(game_mode_retry(1));
    assert!(game_mode_retry(-1));
    assert!(game_mode_retry(i32::MIN));
    assert!(game_mode_retry(i32::MAX));
}

#[test]
fn steps_gaming_split() {
    // a read found its value only with ERROR_SUCCESS
    assert!(gaming_found(ERROR_SUCCESS));
    for status in [
        ERROR_FILE_NOT_FOUND,
        ERROR_ACCESS_DENIED,
        1,
        -1,
        i32::MIN,
        i32::MAX,
    ] {
        assert!(!gaming_found(status));
    }

    // only Game Mode has a line without its value, so only for it is the
    // build read
    assert!(has_fallback(Feature::GameMode));
    for feature in [
        Feature::GameBar,
        Feature::GameDvrAllowed,
        Feature::GameDvr,
        Feature::GameDvrBgRecording,
    ] {
        assert!(!has_fallback(feature));
        // and asked anyway, the others have no fallback line at any build
        assert_eq!(fallback_line(feature, 19042), None);
        assert_eq!(fallback_line(feature, u32::MAX), None);
    }

    // every feature has a line for a found value
    for feature in Feature::ALL {
        let line = value_line(feature, 0);
        assert!(!line.label.is_empty());
        assert!(!line.text.is_empty());
    }
}

#[test]
fn steps_gaming_lines() {
    let ok = ERROR_SUCCESS;
    let line = |f: Feature, value: u32, build: u32| line_of(f as i32, ok, value, build);

    // the same bytes as the five formats it replaces
    assert_eq!(
        line(Feature::GameBar, 1, 0).as_deref(),
        Some("\tGame Bar: On")
    );
    assert_eq!(
        line(Feature::GameBar, 0, 0).as_deref(),
        Some("\tGame Bar: Off")
    );
    assert_eq!(
        line(Feature::GameDvrAllowed, 1, 0).as_deref(),
        Some("\tGame DVR Allowed: Yes")
    );
    assert_eq!(
        line(Feature::GameDvrAllowed, 0, 0).as_deref(),
        Some("\tGame DVR Allowed: No")
    );
    assert_eq!(
        line(Feature::GameDvr, 1, 0).as_deref(),
        Some("\tGame DVR: On")
    );
    assert_eq!(
        line(Feature::GameDvr, 0, 0).as_deref(),
        Some("\tGame DVR: Off")
    );
    assert_eq!(
        line(Feature::GameDvrBgRecording, 1, 0).as_deref(),
        Some("\tGame DVR Background Recording: On")
    );
    assert_eq!(
        line(Feature::GameDvrBgRecording, 0, 0).as_deref(),
        Some("\tGame DVR Background Recording: Off")
    );
    assert_eq!(
        line(Feature::GameMode, 1, 0).as_deref(),
        Some("\tGame Mode: On")
    );
    assert_eq!(
        line(Feature::GameMode, 0, 0).as_deref(),
        Some("\tGame Mode: Off")
    );

    // characterized, not endorsed: any non-zero value is On (or Yes)
    assert_eq!(
        line(Feature::GameBar, 2, 0).as_deref(),
        Some("\tGame Bar: On")
    );
    assert_eq!(
        line(Feature::GameBar, u32::MAX, 0).as_deref(),
        Some("\tGame Bar: On")
    );
    assert_eq!(
        line(Feature::GameDvrAllowed, u32::MAX, 0).as_deref(),
        Some("\tGame DVR Allowed: Yes")
    );
    assert_eq!(
        line(Feature::GameDvr, 0x8000_0000, 0).as_deref(),
        Some("\tGame DVR: On")
    );
    assert_eq!(
        line(Feature::GameDvrBgRecording, 0x100, 0).as_deref(),
        Some("\tGame DVR Background Recording: On")
    );
    assert_eq!(
        line(Feature::GameMode, u32::MAX, 0).as_deref(),
        Some("\tGame Mode: On")
    );

    // a found value ignores the build
    assert_eq!(
        line(Feature::GameMode, 0, 19042).as_deref(),
        Some("\tGame Mode: Off")
    );
    assert_eq!(
        line(Feature::GameBar, 0, u32::MAX).as_deref(),
        Some("\tGame Bar: Off")
    );
}

#[test]
fn steps_failed_reads() {
    let statuses = [
        ERROR_FILE_NOT_FOUND,
        ERROR_ACCESS_DENIED,
        1,
        -1,
        i32::MIN,
        i32::MAX,
    ];

    // the first four features log nothing without their value, at any
    // build and whatever the value
    for status in statuses {
        for feature in Feature::GameBar as i32..=Feature::GameDvrBgRecording as i32 {
            assert_eq!(line_of(feature, status, 0, 0), None);
            assert_eq!(line_of(feature, status, 1, 19042), None);
            assert_eq!(line_of(feature, status, u32::MAX, u32::MAX), None);
        }
    }
}

#[test]
fn steps_game_mode_missing() {
    let mode = Feature::GameMode as i32;
    let probably = Some("\tGame Mode: Probably On (no reg key set)");

    // Game Mode is on by default from build 19042, where its key is often
    // missing
    assert_eq!(line_of(mode, ERROR_FILE_NOT_FOUND, 0, 19041), None);
    assert_eq!(line_of(mode, ERROR_FILE_NOT_FOUND, 1, 0), None);
    assert_eq!(
        line_of(mode, ERROR_FILE_NOT_FOUND, 0, 19042).as_deref(),
        probably
    );
    assert_eq!(
        line_of(mode, ERROR_FILE_NOT_FOUND, 1, 22631).as_deref(),
        probably
    );
    assert_eq!(line_of(mode, -1, 0, u32::MAX).as_deref(), probably);
    // any failure counts as a missing key
    assert_eq!(
        line_of(mode, ERROR_ACCESS_DENIED, 0, 19042).as_deref(),
        probably
    );
}

#[test]
fn steps_unknown_features() {
    // the C's value_line(), fallback() and fallback_line() refuse these
    // too; here no Feature exists for them
    for feature in [-1, 5, 6, i32::MAX, i32::MIN] {
        assert_eq!(Feature::from_index(feature), None);
        assert_eq!(line_of(feature, ERROR_SUCCESS, 1, 19042), None);
        assert_eq!(line_of(feature, ERROR_FILE_NOT_FOUND, 1, 19042), None);
    }
}

#[test]
fn steps_product_labels() {
    assert_eq!(product_state(STATE_ON), c"enabled");
    assert_eq!(product_state(STATE_OFF), c"disabled");
    assert_eq!(product_state(STATE_SNOOZED), c"temporarily disabled");
    assert_eq!(product_state(STATE_EXPIRED), c"expired");
    assert_eq!(product_state(4), c"unknown");
    assert_eq!(product_state(-1), c"unknown");
    assert_eq!(product_state(i32::MAX), c"unknown");
    assert_eq!(product_state(i32::MIN), c"unknown");

    assert_eq!(product_type(PROVIDER_ANTIVIRUS), c"AV");
    assert_eq!(product_type(PROVIDER_FIREWALL), c"FW");
    assert_eq!(product_type(PROVIDER_ANTISPYWARE), c"ASW");
    // WSC_SECURITY_PROVIDER_AUTOUPDATE_SETTINGS and the others the log
    // never asks for
    assert_eq!(product_type(2), c"unknown");
    assert_eq!(product_type(0), c"unknown");
    assert_eq!(product_type(3), c"unknown");
    assert_eq!(product_type(16), c"unknown");
    assert_eq!(product_type(-1), c"unknown");
    assert_eq!(product_type(i32::MIN), c"unknown");
}

#[test]
fn steps_memory_sizes() {
    assert_eq!(mb(0), 0);
    assert_eq!(mb(1), 0);
    assert_eq!(mb(1_048_575), 0);
    assert_eq!(mb(1_048_576), 1);
    assert_eq!(mb(1_048_577), 1);
    assert_eq!(mb(17_179_869_184), 16384);

    // characterized, not endorsed: the size is cut to 32 bits, as the
    // DWORD the log prints (4 PiB and more; unreachable in practice)
    assert_eq!(mb(0xFFFF_FFFF * 1_048_576), u32::MAX);
    assert_eq!(mb(1 << 52), 0);
    assert_eq!(mb((1 << 52) + 1_048_576), 1);
    assert_eq!(mb(u64::MAX), u32::MAX);
}

/// The C's line of a feature, written out from its five branches.
fn model_line(
    feature: Feature,
    status: i32,
    value: u32,
    win_build: u32,
) -> Option<(&'static str, &'static str)> {
    let on = value != 0;
    let found = status == 0;
    match feature {
        Feature::GameBar => found.then_some(("Game Bar", if on { "On" } else { "Off" })),
        Feature::GameDvrAllowed => {
            found.then_some(("Game DVR Allowed", if on { "Yes" } else { "No" }))
        }
        Feature::GameDvr => found.then_some(("Game DVR", if on { "On" } else { "Off" })),
        Feature::GameDvrBgRecording => found.then_some((
            "Game DVR Background Recording",
            if on { "On" } else { "Off" },
        )),
        Feature::GameMode => {
            if found {
                Some(("Game Mode", if on { "On" } else { "Off" }))
            } else if win_build >= 19042 {
                Some(("Game Mode", "Probably On (no reg key set)"))
            } else {
                None
            }
        }
    }
}

fn status() -> impl Strategy<Value = i32> {
    prop_oneof![Just(0), Just(ERROR_FILE_NOT_FOUND), any::<i32>()]
}

fn value() -> impl Strategy<Value = u32> {
    prop_oneof![Just(0), Just(1), any::<u32>()]
}

fn build() -> impl Strategy<Value = u32> {
    prop_oneof![19040u32..19045, any::<u32>()]
}

#[test]
fn edge_feature_numbers() {
    for (i, feature) in Feature::ALL.into_iter().enumerate() {
        let index = i32::try_from(i).expect("five features");
        assert_eq!(feature as i32, index);
        assert_eq!(Feature::from_index(index), Some(feature));
    }
    assert_eq!(FEATURES[Feature::GameMode as usize].label, c"Game Mode");
    assert!(FEATURES.iter().take(4).all(|rule| rule.missing.is_none()));
}

#[test]
fn edge_build_threshold() {
    let mode = Feature::GameMode;
    for build in [0, 19040, 19041] {
        assert_eq!(fallback_line(mode, build), None);
    }
    for build in [19042, 19043, 22000, u32::MAX] {
        let line = fallback_line(mode, build).expect("logged");
        assert_eq!(line.label, c"Game Mode");
        assert_eq!(line.text, c"Probably On (no reg key set)");
    }
}

#[test]
fn edge_version_gate() {
    for (major, minor, logged) in [(6, 1, false), (6, 3, false), (9, 255, false), (10, 0, true)] {
        assert_eq!(gaming_logged(version(major, minor)), logged);
    }
    assert!(!gaming_logged(GAMING_MIN_VERSION - 1));
    assert!(gaming_logged(GAMING_MIN_VERSION));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(4096)))]

    #[test]
    fn edge_version_any(major in any::<u32>(), minor in any::<u32>()) {
        prop_assert_eq!(version(major, minor), major.wrapping_shl(8) | minor);
    }

    #[test]
    fn edge_mb_any(bytes in any::<u64>()) {
        let expected = u32::try_from((bytes / 1_048_576) & 0xFFFF_FFFF).expect("masked");
        prop_assert_eq!(mb(bytes), expected);
    }

    #[test]
    fn edge_gate_and_retry_any(win_ver in any::<u32>(), status in any::<i32>()) {
        prop_assert_eq!(gaming_logged(win_ver), win_ver >= 0xA00);
        prop_assert_eq!(game_mode_retry(status), status != 0);
    }

    #[test]
    fn edge_line_any(
        index in 0usize..5,
        status in status(),
        value in value(),
        win_build in build(),
    ) {
        let feature = Feature::ALL[index];
        let got = gaming_line(feature, status, value, win_build)
            .map(|line| (text(line.label), text(line.text)));
        prop_assert_eq!(got, model_line(feature, status, value, win_build));
        prop_assert_eq!(gaming_found(status), status == 0);
        prop_assert_eq!(has_fallback(feature), feature == Feature::GameMode);
        let fallback = fallback_line(feature, win_build).map(|line| (text(line.label), text(line.text)));
        let expected = (feature == Feature::GameMode && win_build >= 19042)
            .then_some(("Game Mode", "Probably On (no reg key set)"));
        prop_assert_eq!(fallback, expected);
    }

    #[test]
    fn edge_feature_index_any(index in any::<i32>()) {
        let expected = usize::try_from(index).ok().and_then(|i| Feature::ALL.get(i).copied());
        prop_assert_eq!(Feature::from_index(index), expected);
    }

    #[test]
    fn edge_labels_any(state in any::<i32>(), kind in any::<i32>()) {
        let s = match state {
            0 => c"enabled",
            1 => c"disabled",
            2 => c"temporarily disabled",
            3 => c"expired",
            _ => UNKNOWN,
        };
        let t = match kind {
            4 => c"AV",
            1 => c"FW",
            8 => c"ASW",
            _ => UNKNOWN,
        };
        prop_assert_eq!(product_state(state), s);
        prop_assert_eq!(product_type(kind), t);
    }
}
