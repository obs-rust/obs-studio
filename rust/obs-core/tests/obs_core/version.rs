//! Tier 1: the version packing and the numbers read from `obs-config.h`.
use obs_core::version::{
    LIBOBS_API_MAJOR_VER, LIBOBS_API_MINOR_VER, LIBOBS_API_PATCH_VER, LIBOBS_API_VER,
    OBS_CONFIG_H, define_value, make_semantic_version,
};

#[test]
fn packing_is_major_minor_patch() {
    assert_eq!(make_semantic_version(0, 0, 0), 0);
    assert_eq!(make_semantic_version(33, 0, 0), 0x2100_0000);
    assert_eq!(make_semantic_version(30, 2, 3), 0x1e02_0003);
    assert_eq!(make_semantic_version(1, 255, 255), 0x01ff_00ff);
    assert_eq!(make_semantic_version(255, 0, 0), 0xff00_0000);
    // A patch past 16 bits runs into the minor, as the C's `|` does.
    assert_eq!(make_semantic_version(0, 1, 0x1_0000), 0x0001_0000);
    assert_eq!(make_semantic_version(0, 0, u32::MAX), u32::MAX);
    // Bits shifted past the top are dropped.
    assert_eq!(make_semantic_version(0x1ff, 0, 0), 0xff00_0000);
}

#[test]
fn numbers_come_from_obs_config_h() {
    assert_eq!(define_value(OBS_CONFIG_H, "LIBOBS_API_MAJOR_VER"), Some(LIBOBS_API_MAJOR_VER));
    assert_eq!(define_value(OBS_CONFIG_H, "LIBOBS_API_MINOR_VER"), Some(LIBOBS_API_MINOR_VER));
    assert_eq!(define_value(OBS_CONFIG_H, "LIBOBS_API_PATCH_VER"), Some(LIBOBS_API_PATCH_VER));
    assert!(LIBOBS_API_MAJOR_VER > 0);
    assert_eq!(
        LIBOBS_API_VER,
        make_semantic_version(LIBOBS_API_MAJOR_VER, LIBOBS_API_MINOR_VER, LIBOBS_API_PATCH_VER)
    );
}

#[test]
fn define_value_reads_only_the_named_decimal_define() {
    let header = "/* #define A 9 */\n#define AB 7\n#define A 12\n#define B  4 \n#define C x\n";
    assert_eq!(define_value(header, "A"), Some(12));
    assert_eq!(define_value(header, "AB"), Some(7));
    assert_eq!(define_value(header, "B"), Some(4));
    assert_eq!(define_value(header, "C"), None);
    assert_eq!(define_value(header, "D"), None);
    assert_eq!(define_value("#define A 4294967296\n", "A"), None);
    assert_eq!(define_value("#define A 4294967295", "A"), Some(u32::MAX));
    assert_eq!(define_value("", "A"), None);
}

#[test]
fn version_getters_match_the_c_characterization() {
    // test_version: the version is LIBOBS_API_VER; the string is non-empty
    // and the same pointer on each call.
    assert_ne!(LIBOBS_API_VER, 0);
    assert_eq!(obs_core::ffi::version::obs_get_version(), LIBOBS_API_VER);
    assert_eq!(obs_core::ffi::version::obs_get_version(), LIBOBS_API_VER);
}
