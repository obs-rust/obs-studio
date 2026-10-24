//! obs-core: the output, encoder and service registration rules
//! (`libobs/obs-module-type-rules.c.h`), one call each, as the registrations
//! make them for one type: an RTMP-like output (service, encoded, audio and
//! video: every output rule up to encoded_packet), a texture video encoder
//! and a plain audio encoder (every encoder rule), a service, and the
//! protocol dedupe of a service output against a registered list where the
//! protocol is last. Each runs once per registered type (the dedupe once per
//! protocol), at startup and when a module loads. The protocols are owned
//! C strings that the module frees when it returns.

use std::ffi::{CString, c_char};

use criterion::Criterion;
use obs_c_oracle::module_type_rules::{
    oracle_obs_module_type_rules_encoder_check, oracle_obs_module_type_rules_output_check,
    oracle_obs_module_type_rules_protocol_listed, oracle_obs_module_type_rules_service_check,
};
use obs_core::ffi::module_type_rules::{
    obs_module_type_rules_encoder_check, obs_module_type_rules_output_check,
    obs_module_type_rules_protocol_listed, obs_module_type_rules_service_check,
};
use obs_core::module_type_rules::{
    CAP_PASS_TEXTURE, ENCODER_AUDIO, ENCODER_VIDEO, OUTPUT_AUDIO, OUTPUT_ENCODED, OUTPUT_SERVICE,
    OUTPUT_VIDEO,
};

/// Every callback bit set.
const ALL: u32 = (1 << 15) - 1;
const RTMP_LIKE: u32 = OUTPUT_VIDEO | OUTPUT_AUDIO | OUTPUT_ENCODED | OUTPUT_SERVICE;

pub fn benchmarks(criterion: &mut Criterion) {
    super::pair(
        criterion,
        "module_type_rules/output_check",
        || obs_module_type_rules_output_check(RTMP_LIKE, ALL),
        // SAFETY: plain values.
        || unsafe { oracle_obs_module_type_rules_output_check(RTMP_LIKE, ALL) },
    );
    for (case, encoder_type, caps) in [
        ("texture_video", ENCODER_VIDEO, CAP_PASS_TEXTURE),
        ("audio", ENCODER_AUDIO, 0),
    ] {
        super::pair(
            criterion,
            &format!("module_type_rules/encoder_check/{case}"),
            || obs_module_type_rules_encoder_check(encoder_type, caps, ALL),
            // SAFETY: plain values.
            || unsafe { oracle_obs_module_type_rules_encoder_check(encoder_type, caps, ALL) },
        );
    }
    super::pair(
        criterion,
        "module_type_rules/service_check",
        || obs_module_type_rules_service_check(ALL),
        // SAFETY: plain values.
        || unsafe { oracle_obs_module_type_rules_service_check(ALL) },
    );

    let owned: Vec<CString> = ["RTMP", "RTMPS", "SRT", "RIST", "WHIP", "HLS"]
        .iter()
        .map(|p| CString::new(*p).unwrap())
        .collect();
    let list: Vec<*mut c_char> = owned.iter().map(|p| p.as_ptr().cast_mut()).collect();
    let protocol = c"HLS";
    super::pair(
        criterion,
        "module_type_rules/protocol_listed",
        // SAFETY: NUL-terminated strings; `list.len()` entries.
        || unsafe {
            obs_module_type_rules_protocol_listed(protocol.as_ptr(), list.as_ptr(), list.len())
        },
        // SAFETY: as above.
        || unsafe {
            oracle_obs_module_type_rules_protocol_listed(
                protocol.as_ptr(),
                list.as_ptr(),
                list.len(),
            )
        },
    );
}
