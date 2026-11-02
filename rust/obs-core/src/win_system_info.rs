//! Safe core of `libobs/obs-windows-system-info.c.h`: the rules of
//! `libobs/obs-windows.c.h`'s system info log.
//!
//! - [`version`]: `log_system_info()`'s packed `win_ver`;
//! - [`mb`]: `log_available_memory()`'s sizes in MB;
//! - [`gaming_logged`], [`game_mode_retry`], [`gaming_found`],
//!   [`value_line`], [`has_fallback`] and [`fallback_line`]:
//!   `log_gaming_features()`'s Windows 10 gate, whether Game Mode is read a
//!   second time, whether each read found its value, and each feature's
//!   line, from [`FEATURES`]. The C reads a value only once its read is
//!   found, and the Windows build only for a feature with a fallback, so the
//!   line is split the same way;
//! - [`product_state`] and [`product_type`]: the labels
//!   `log_security_products_by_type()` logs for a security product.
//!
//! The C keeps `get_win_ver()`, `GlobalMemoryStatusEx()`, every registry
//! read, the COM calls, the 32-bit memory note, every `blog()` call and
//! format string, and the `win_ver` and `win_build` statics, each at its
//! point, and passes their values in. Nothing here keeps state or
//! allocates; the labels are `'static` C strings the C logs as they are.
//!
//! The core is OS-free: it takes the C's integer values.
//!
//! Characterized, not endorsed (`test_windows_system_info_steps`), and
//! kept: any non-zero registry value counts as On or Yes; a minor version of
//! 256 or more runs into the major; the MB sizes are cut to 32 bits.
//!
//! No intentional differences from the C.

use std::ffi::CStr;

/// `winerror.h`'s `ERROR_SUCCESS`, the status of a registry read that found
/// its value.
pub const ERROR_SUCCESS: i32 = 0;

/// `iwscapi.h`'s `WSC_SECURITY_PRODUCT_STATE` values.
pub const STATE_ON: i32 = 0;
pub const STATE_OFF: i32 = 1;
pub const STATE_SNOOZED: i32 = 2;
pub const STATE_EXPIRED: i32 = 3;

/// `wscapi.h`'s `WSC_SECURITY_PROVIDER` values the log asks for.
pub const PROVIDER_FIREWALL: i32 = 0x1;
pub const PROVIDER_ANTIVIRUS: i32 = 0x4;
pub const PROVIDER_ANTISPYWARE: i32 = 0x8;

/// The first Windows version whose gaming features are logged: 10.0
/// (Windows 11 reports 10.0 too).
pub const GAMING_MIN_VERSION: u32 = 0xA00;

/// The gaming features `log_gaming_features()` reads, in the order it logs
/// them; the value is the C's feature number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Feature {
    GameBar = 0,
    GameDvrAllowed = 1,
    GameDvr = 2,
    GameDvrBgRecording = 3,
    GameMode = 4,
}

impl Feature {
    /// Every feature, in logged order.
    pub const ALL: [Self; 5] = [
        Self::GameBar,
        Self::GameDvrAllowed,
        Self::GameDvr,
        Self::GameDvrBgRecording,
        Self::GameMode,
    ];

    /// The feature with the C's number, if any.
    pub fn from_index(index: i32) -> Option<Self> {
        let _ = index;
        todo!()
    }
}

/// A feature's line when its value is missing, from a Windows build on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Missing {
    /// The first build that logs it.
    pub from_build: u32,
    /// The text logged.
    pub text: &'static CStr,
}

/// How one feature's line reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeatureRule {
    pub label: &'static CStr,
    /// The text for a non-zero value.
    pub on: &'static CStr,
    /// The text for zero.
    pub off: &'static CStr,
    /// The line logged when the value is missing, if any.
    pub missing: Option<Missing>,
}

/// The five features' lines, indexed by [`Feature`].
pub const FEATURES: [FeatureRule; 5] = [
    FeatureRule {
        label: c"Game Bar",
        on: c"On",
        off: c"Off",
        missing: None,
    },
    FeatureRule {
        label: c"Game DVR Allowed",
        on: c"Yes",
        off: c"No",
        missing: None,
    },
    FeatureRule {
        label: c"Game DVR",
        on: c"On",
        off: c"Off",
        missing: None,
    },
    FeatureRule {
        label: c"Game DVR Background Recording",
        on: c"On",
        off: c"Off",
        missing: None,
    },
    // On by default in newer Windows 10 builds (no registry key set).
    FeatureRule {
        label: c"Game Mode",
        on: c"On",
        off: c"Off",
        missing: Some(Missing {
            from_build: 19042,
            text: c"Probably On (no reg key set)",
        }),
    },
];

/// A feature's logged line, `"\t<label>: <text>"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line {
    pub label: &'static CStr,
    pub text: &'static CStr,
}

/// The label for an unknown product state or type.
pub const UNKNOWN: &CStr = c"unknown";

/// `win_ver`: `(major << 8) | minor`, in 32 bits.
pub fn version(major: u32, minor: u32) -> u32 {
    let _ = (major, minor);
    todo!()
}

/// A size in MB, cut to 32 bits as the `DWORD` the log prints.
pub fn mb(bytes: u64) -> u32 {
    let _ = bytes;
    todo!()
}

/// Whether the gaming features are logged: Windows 10 and later.
pub fn gaming_logged(win_ver: u32) -> bool {
    let _ = win_ver;
    todo!()
}

/// Whether Game Mode is read again under its older value name: the first
/// read failed.
pub fn game_mode_retry(status: i32) -> bool {
    let _ = status;
    todo!()
}

/// Whether a feature's read found its value: its status is
/// `ERROR_SUCCESS`. Only then does the C read the value.
pub fn gaming_found(status: i32) -> bool {
    let _ = status;
    todo!()
}

/// The line of a feature whose read found its value: its on text for any
/// non-zero value (the C's `(bool)value`), its off text for zero.
pub fn value_line(feature: Feature, value: u32) -> Line {
    let _ = (feature, value);
    todo!()
}

/// Whether a feature whose value is missing may still get a line (Game
/// Mode). Only then does the C read the Windows build.
pub fn has_fallback(feature: Feature) -> bool {
    let _ = feature;
    todo!()
}

/// The line of a feature whose value is missing, from the Windows build, or
/// `None`.
pub fn fallback_line(feature: Feature, win_build: u32) -> Option<Line> {
    let _ = (feature, win_build);
    todo!()
}

/// The label of a security product's state.
pub fn product_state(state: i32) -> &'static CStr {
    let _ = state;
    todo!()
}

/// The label of a security product's type.
pub fn product_type(kind: i32) -> &'static CStr {
    let _ = kind;
    todo!()
}
