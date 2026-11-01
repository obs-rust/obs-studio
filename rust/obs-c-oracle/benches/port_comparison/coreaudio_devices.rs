//! obs-audio-monitoring: the CoreAudio monitoring device rules
//! (`libobs/audio-monitoring/osx/coreaudio-device-rules.c.h`). They run for
//! each device of every walk of the device list: the monitoring device
//! list, `devices_match()` (twice per call) and the desktop audio default.
//! Each side walks 8 devices as those ask: whether each is listed, whether
//! its uid names a virtual device, the "default" test of the id looked for
//! and the name match, with sizes both calls agree on, so the one
//! intentional difference is not asked. The strings are constants, nothing
//! is allocated, and the results fold into a checksum both must match.

use super::pair;

use criterion::Criterion;
use obs_audio_monitoring::ffi::coreaudio_devices::{
    coreaudio_device_count, coreaudio_device_is_default, coreaudio_device_is_input,
    coreaudio_device_listed, coreaudio_device_names_match,
};
use obs_c_oracle::coreaudio_device_rules::{
    oracle_coreaudio_device_count, oracle_coreaudio_device_is_default,
    oracle_coreaudio_device_is_input, oracle_coreaudio_device_listed,
    oracle_coreaudio_device_names_match,
};
use std::ffi::{CStr, c_char};
use std::hint::black_box;

/// A device list as a Mac with a few audio tools shows it: uid, name and
/// output stream bytes.
const DEVICES: [(&CStr, &CStr, u32); 8] = [
    (c"BuiltInSpeakerDevice", c"MacBook Pro Speakers", 8),
    (c"BuiltInMicrophoneDevice", c"MacBook Pro Microphone", 0),
    (c"BlackHole2ch_UID", c"BlackHole 2ch", 8),
    (c"AppleUSBAudioEngine:Generic:USB Audio:14200000:2", c"USB Audio", 8),
    (c"com.rogueamoeba.Loopback:A1B2C3", c"Loopback Audio", 8),
    (c"AppleHDAEngineOutputDP:3,0,1,0:0:{2D4C-9A1B}", c"LG UltraFine", 8),
    (c"ZoomAudioDevice", c"ZoomAudioDevice", 8),
    (c"VBAudioVACDevice_VBCable", c"VB-Cable", 8),
];
const IDS: [&CStr; 2] = [c"default", c"BlackHole2ch_UID"];

fn mix(acc: u64, value: u64) -> u64 {
    acc.wrapping_mul(31).wrapping_add(value)
}

/// The device rules of one side.
struct Side {
    listed: fn(bool, u32) -> bool,
    count: fn(u32, u32, usize) -> u32,
    is_default: fn(*const c_char) -> bool,
    names_match: fn(*const c_char, *const c_char) -> bool,
    is_input: fn(*const c_char) -> bool,
}

fn walk(s: &Side) -> u64 {
    let size = black_box(u32::try_from(DEVICES.len() * 4).unwrap_or(0));
    let n = (s.count)(size, size, 4) as usize;
    let mut acc = mix(0, n as u64);
    for (k, id) in IDS.iter().enumerate() {
        acc = mix(acc, u64::from((s.is_default)(black_box(id.as_ptr()))));
        let allow = k == 1;
        for (uid, name, streams) in DEVICES.iter().take(n) {
            if !(s.listed)(allow, black_box(*streams)) {
                continue;
            }
            acc = mix(acc, u64::from((s.is_input)(black_box(uid.as_ptr()))));
            acc = mix(acc, u64::from((s.names_match)(name.as_ptr(), black_box(DEVICES[2].1.as_ptr()))));
        }
    }
    acc
}

fn rust_walk() -> u64 {
    // SAFETY (each closure): constant NUL-terminated strings.
    walk(&Side {
        listed: |a, b| coreaudio_device_listed(a, b),
        count: |f, s, i| coreaudio_device_count(f, s, i),
        is_default: |id| unsafe { coreaudio_device_is_default(id) },
        names_match: |a, b| unsafe { coreaudio_device_names_match(a, b) },
        is_input: |u| unsafe { coreaudio_device_is_input(u) },
    })
}

fn c_walk() -> u64 {
    // SAFETY (each closure): plain values and constant NUL-terminated
    // strings; the oracle reads nothing else.
    walk(&Side {
        listed: |a, b| unsafe { oracle_coreaudio_device_listed(a, b) },
        count: |f, s, i| unsafe { oracle_coreaudio_device_count(f, s, i) },
        is_default: |id| unsafe { oracle_coreaudio_device_is_default(id) },
        names_match: |a, b| unsafe { oracle_coreaudio_device_names_match(a, b) },
        is_input: |u| unsafe { oracle_coreaudio_device_is_input(u) },
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(criterion, "coreaudio_devices/walk/8", rust_walk, c_walk);
}
