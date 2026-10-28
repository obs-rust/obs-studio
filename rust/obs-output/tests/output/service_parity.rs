//! Tier 3 for the service getters (#1899): the Rust shims in
//! obs_output::ffi::service against the original C
//! (rust/obs-c-oracle/oracle/service_getters.c), on the same
//! `struct obs_service`.
//!
//! Every combination of the 8 optional callbacks (256 tables) runs every
//! getter with and without each out-parameter, with sentinel values in
//! them; both sides must return the same value, leave the same
//! out-parameters and make the same plugin calls, with the same data
//! pointer and arguments. The mirror of `struct obs_service_info` must have
//! the C's size, alignment and field offsets.
//!
//! Not compared: the NULL-parameter debug log lines (Tier 1 checks their
//! function and parameter names), and the codec getters with a NULL service,
//! which crash on both sides.

use core::cell::RefCell;
use core::ffi::{c_char, c_int, c_void};
use core::mem::{align_of, offset_of, size_of};
use core::ptr;

use obs_c_oracle::service_getters as c;
use obs_output::ffi::service::{self as rust, ObsServiceInfo, ObsServiceResolution};

thread_local! {
    static TRACE: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn trace(event: String) {
    TRACE.with(|t| t.borrow_mut().push(event));
}

fn take_trace() -> Vec<String> {
    TRACE.with(|t| core::mem::take(&mut *t.borrow_mut()))
}

static PROTOCOL: &[u8] = b"SRT\0";
static OUTPUT: &[u8] = b"ffmpeg_mpegts_muxer\0";
static URL: &[u8] = b"srt://example.invalid\0";

struct Lists {
    video: [*const c_char; 2],
    audio: [*const c_char; 2],
}
// SAFETY: pointers to static strings, read only.
unsafe impl Sync for Lists {}
static LISTS: Lists = Lists {
    video: [c"h264".as_ptr(), ptr::null()],
    audio: [c"aac".as_ptr(), ptr::null()],
};
static mut RESOLUTION: ObsServiceResolution = ObsServiceResolution { cx: 1920, cy: 1080 };

unsafe extern "C" fn get_name(_: *mut c_void) -> *const c_char {
    c"parity".as_ptr()
}
unsafe extern "C" fn create(_: *mut c_void, _: *mut c_void) -> *mut c_void {
    ptr::null_mut()
}
unsafe extern "C" fn destroy(_: *mut c_void) {}
unsafe extern "C" fn protocol(data: *mut c_void) -> *const c_char {
    trace(format!("protocol {data:p}"));
    PROTOCOL.as_ptr().cast()
}
unsafe extern "C" fn connect_info(data: *mut c_void, kind: u32) -> *const c_char {
    trace(format!("connect_info {data:p} {kind}"));
    if kind == 0 {
        URL.as_ptr().cast()
    } else {
        ptr::null()
    }
}
unsafe extern "C" fn can_try(data: *mut c_void) -> bool {
    trace(format!("can_try {data:p}"));
    false
}
unsafe extern "C" fn output_type(data: *mut c_void) -> *const c_char {
    trace(format!("output_type {data:p}"));
    OUTPUT.as_ptr().cast()
}
unsafe extern "C" fn max_fps(data: *mut c_void, fps: *mut c_int) {
    // SAFETY: the getter passes a writable fps.
    let before = unsafe { *fps };
    trace(format!("max_fps {data:p} {before}"));
    unsafe { *fps = 60 };
}
unsafe extern "C" fn max_bitrate(data: *mut c_void, video: *mut c_int, audio: *mut c_int) {
    // SAFETY: NULL or writable, as the getter got them.
    let (v, a) = unsafe { (video.as_mut(), audio.as_mut()) };
    trace(format!(
        "max_bitrate {data:p} {:?} {:?}",
        v.as_deref(),
        a.as_deref()
    ));
    if let Some(v) = v {
        *v = 6000;
    }
}
unsafe extern "C" fn resolutions(
    data: *mut c_void,
    res: *mut *mut ObsServiceResolution,
    count: *mut usize,
) {
    // SAFETY: writable, as the getter checked.
    unsafe {
        trace(format!("resolutions {data:p} {:p} {}", *res, *count));
        *res = &raw mut RESOLUTION;
        *count = 1;
    }
}
unsafe extern "C" fn video(data: *mut c_void) -> *mut *const c_char {
    trace(format!("video {data:p}"));
    LISTS.video.as_ptr().cast_mut()
}
unsafe extern "C" fn audio(data: *mut c_void) -> *mut *const c_char {
    trace(format!("audio {data:p}"));
    LISTS.audio.as_ptr().cast_mut()
}

fn table(mask: u32) -> ObsServiceInfo {
    let has = |bit: u32| mask & (1 << bit) != 0;
    ObsServiceInfo {
        id: c"parity_service".as_ptr(),
        get_name: Some(get_name),
        create: Some(create),
        destroy: Some(destroy),
        activate: None,
        deactivate: None,
        update: None,
        get_defaults: None,
        get_properties: None,
        initialize: None,
        get_url: None,
        get_key: None,
        get_username: None,
        get_password: None,
        deprecated_1: None,
        apply_encoder_settings: None,
        type_data: ptr::null_mut(),
        free_type_data: None,
        get_output_type: has(0).then_some(output_type as _),
        get_supported_resolutions: has(1).then_some(resolutions as _),
        get_max_fps: has(2).then_some(max_fps as _),
        get_max_bitrate: has(3).then_some(max_bitrate as _),
        get_supported_video_codecs: has(4).then_some(video as _),
        get_protocol: Some(protocol),
        get_supported_audio_codecs: has(5).then_some(audio as _),
        get_connect_info: has(6).then_some(connect_info as _),
        can_try_to_connect: has(7).then_some(can_try as _),
    }
}

/// One getter call's observable result: its return value, out-parameters
/// and plugin calls.
type Seen = (String, Vec<String>);

fn run(f: impl FnOnce() -> String) -> Seen {
    take_trace();
    let out = f();
    (out, take_trace())
}

/// Runs `call` on the C and on the Rust with the same service; both must
/// agree.
fn same(
    what: &str,
    mask: u32,
    service: *const c_void,
    call: impl Fn(bool, *const c_void) -> String,
) {
    let c_side = run(|| call(false, service));
    let rust_side = run(|| call(true, service));
    assert_eq!(rust_side, c_side, "{what}, callback mask {mask:#010b}");
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "parity table: one block per ported C case"
)]
fn every_callback_table_matches_the_c() {
    let mut data = 0x5e4c_u32;
    let data = (&raw mut data).cast::<c_void>();
    for mask in 0..256 {
        let info = table(mask);
        // SAFETY: a full table and a live data pointer for the service's life.
        let service = unsafe { c::oracle_service_new((&raw const info).cast(), data) }.cast_const();
        for svc in [service, ptr::null()] {
            same("protocol", mask, svc, |r, s| unsafe {
                format!(
                    "{:p}",
                    if r {
                        rust::obs_service_get_protocol(s)
                    } else {
                        c::oracle_obs_service_get_protocol(s)
                    }
                )
            });
            for kind in [0, 1, 7, u32::MAX] {
                same("connect_info", mask, svc, |r, s| unsafe {
                    format!(
                        "{:p}",
                        if r {
                            rust::obs_service_get_connect_info(s, kind)
                        } else {
                            c::oracle_obs_service_get_connect_info(s, kind)
                        }
                    )
                });
            }
            same("can_try_to_connect", mask, svc, |r, s| unsafe {
                format!(
                    "{}",
                    if r {
                        rust::obs_service_can_try_to_connect(s)
                    } else {
                        c::oracle_obs_service_can_try_to_connect(s)
                    }
                )
            });
            same("preferred_output_type", mask, svc, |r, s| unsafe {
                format!(
                    "{:p}",
                    if r {
                        rust::obs_service_get_preferred_output_type(s)
                    } else {
                        c::oracle_obs_service_get_preferred_output_type(s)
                    }
                )
            });
            for give in [true, false] {
                same("max_fps", mask, svc, |r, s| unsafe {
                    let mut fps: c_int = -7;
                    let p = if give { &raw mut fps } else { ptr::null_mut() };
                    if r {
                        rust::obs_service_get_max_fps(s, p)
                    } else {
                        c::oracle_obs_service_get_max_fps(s, p)
                    }
                    format!("{fps}")
                });
            }
            for (gv, ga) in [(true, true), (true, false), (false, true), (false, false)] {
                same("max_bitrate", mask, svc, |r, s| unsafe {
                    let (mut v, mut a): (c_int, c_int) = (-7, -9);
                    let pv = if gv { &raw mut v } else { ptr::null_mut() };
                    let pa = if ga { &raw mut a } else { ptr::null_mut() };
                    if r {
                        rust::obs_service_get_max_bitrate(s, pv, pa)
                    } else {
                        c::oracle_obs_service_get_max_bitrate(s, pv, pa)
                    }
                    format!("{v} {a}")
                });
                same("supported_resolutions", mask, svc, |r, s| unsafe {
                    // A sentinel: never dereferenced, only compared.
                    let mut res = ptr::without_provenance_mut::<ObsServiceResolution>(0x1);
                    let mut count: usize = 7;
                    let pr = if gv { &raw mut res } else { ptr::null_mut() };
                    let pc = if ga { &raw mut count } else { ptr::null_mut() };
                    if r {
                        rust::obs_service_get_supported_resolutions(s, pr, pc)
                    } else {
                        c::oracle_obs_service_get_supported_resolutions(s, pr.cast(), pc)
                    }
                    format!("{res:p} {count}")
                });
            }
            if !svc.is_null() {
                same("video_codecs", mask, svc, |r, s| unsafe {
                    format!(
                        "{:p}",
                        if r {
                            rust::obs_service_get_supported_video_codecs(s)
                        } else {
                            c::oracle_obs_service_get_supported_video_codecs(s)
                        }
                    )
                });
                same("audio_codecs", mask, svc, |r, s| unsafe {
                    format!(
                        "{:p}",
                        if r {
                            rust::obs_service_get_supported_audio_codecs(s)
                        } else {
                            c::oracle_obs_service_get_supported_audio_codecs(s)
                        }
                    )
                });
            }
        }
        // SAFETY: from oracle_service_new, freed once.
        unsafe { c::oracle_service_free(service.cast_mut()) };
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "parity table: one block per ported C case"
)]
fn service_info_mirror_has_the_c_layout() {
    // SAFETY: the oracle's layout getters take no arguments.
    unsafe {
        assert_eq!(
            size_of::<ObsServiceResolution>(),
            c::oracle_service_resolution_size()
        );
        assert_eq!(
            align_of::<ObsServiceResolution>(),
            c::oracle_service_resolution_align()
        );
        assert_eq!(
            offset_of!(ObsServiceResolution, cy),
            c::oracle_service_resolution_offset_cy()
        );
        assert_eq!(size_of::<ObsServiceInfo>(), c::oracle_service_info_size());
        assert_eq!(align_of::<ObsServiceInfo>(), c::oracle_service_info_align());
    }
    let offsets: [(&str, usize, usize); 27] = unsafe {
        [
            (
                "id",
                offset_of!(ObsServiceInfo, id),
                c::oracle_service_info_offset_id(),
            ),
            (
                "get_name",
                offset_of!(ObsServiceInfo, get_name),
                c::oracle_service_info_offset_get_name(),
            ),
            (
                "create",
                offset_of!(ObsServiceInfo, create),
                c::oracle_service_info_offset_create(),
            ),
            (
                "destroy",
                offset_of!(ObsServiceInfo, destroy),
                c::oracle_service_info_offset_destroy(),
            ),
            (
                "activate",
                offset_of!(ObsServiceInfo, activate),
                c::oracle_service_info_offset_activate(),
            ),
            (
                "deactivate",
                offset_of!(ObsServiceInfo, deactivate),
                c::oracle_service_info_offset_deactivate(),
            ),
            (
                "update",
                offset_of!(ObsServiceInfo, update),
                c::oracle_service_info_offset_update(),
            ),
            (
                "get_defaults",
                offset_of!(ObsServiceInfo, get_defaults),
                c::oracle_service_info_offset_get_defaults(),
            ),
            (
                "get_properties",
                offset_of!(ObsServiceInfo, get_properties),
                c::oracle_service_info_offset_get_properties(),
            ),
            (
                "initialize",
                offset_of!(ObsServiceInfo, initialize),
                c::oracle_service_info_offset_initialize(),
            ),
            (
                "get_url",
                offset_of!(ObsServiceInfo, get_url),
                c::oracle_service_info_offset_get_url(),
            ),
            (
                "get_key",
                offset_of!(ObsServiceInfo, get_key),
                c::oracle_service_info_offset_get_key(),
            ),
            (
                "get_username",
                offset_of!(ObsServiceInfo, get_username),
                c::oracle_service_info_offset_get_username(),
            ),
            (
                "get_password",
                offset_of!(ObsServiceInfo, get_password),
                c::oracle_service_info_offset_get_password(),
            ),
            (
                "deprecated_1",
                offset_of!(ObsServiceInfo, deprecated_1),
                c::oracle_service_info_offset_deprecated_1(),
            ),
            (
                "apply_encoder_settings",
                offset_of!(ObsServiceInfo, apply_encoder_settings),
                c::oracle_service_info_offset_apply_encoder_settings(),
            ),
            (
                "type_data",
                offset_of!(ObsServiceInfo, type_data),
                c::oracle_service_info_offset_type_data(),
            ),
            (
                "free_type_data",
                offset_of!(ObsServiceInfo, free_type_data),
                c::oracle_service_info_offset_free_type_data(),
            ),
            (
                "get_output_type",
                offset_of!(ObsServiceInfo, get_output_type),
                c::oracle_service_info_offset_get_output_type(),
            ),
            (
                "get_supported_resolutions",
                offset_of!(ObsServiceInfo, get_supported_resolutions),
                c::oracle_service_info_offset_get_supported_resolutions(),
            ),
            (
                "get_max_fps",
                offset_of!(ObsServiceInfo, get_max_fps),
                c::oracle_service_info_offset_get_max_fps(),
            ),
            (
                "get_max_bitrate",
                offset_of!(ObsServiceInfo, get_max_bitrate),
                c::oracle_service_info_offset_get_max_bitrate(),
            ),
            (
                "get_supported_video_codecs",
                offset_of!(ObsServiceInfo, get_supported_video_codecs),
                c::oracle_service_info_offset_get_supported_video_codecs(),
            ),
            (
                "get_protocol",
                offset_of!(ObsServiceInfo, get_protocol),
                c::oracle_service_info_offset_get_protocol(),
            ),
            (
                "get_supported_audio_codecs",
                offset_of!(ObsServiceInfo, get_supported_audio_codecs),
                c::oracle_service_info_offset_get_supported_audio_codecs(),
            ),
            (
                "get_connect_info",
                offset_of!(ObsServiceInfo, get_connect_info),
                c::oracle_service_info_offset_get_connect_info(),
            ),
            (
                "can_try_to_connect",
                offset_of!(ObsServiceInfo, can_try_to_connect),
                c::oracle_service_info_offset_can_try_to_connect(),
            ),
        ]
    };
    for (field, rust, c) in offsets {
        assert_eq!(rust, c, "offset of {field}");
    }
}
