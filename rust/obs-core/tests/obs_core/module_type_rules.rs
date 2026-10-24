//! Tier 1: the safe core of `libobs/obs-module-type-rules.c.h`.
//!
//! The first sixteen tests mirror the cases of
//! `test/cmocka/test_module_type_rules_steps.c` one to one, in the same
//! order, with the same ids, sizes and expected messages. The first six call
//! the rules directly, as the C test calls the moved functions. The last ten
//! go through [`Registry`], which stands for `obs_register_output_s()`,
//! `obs_register_encoder_s()` and `obs_register_service_s()`: it looks for a
//! duplicate, finds the callbacks inside the size (`HAS_VAL()`), asks the
//! rules, checks the size with slice 4's [`size_supported`], copies the part
//! of the structure inside the size and merges the protocols with
//! [`protocol_listed`], logging what the C logs and freeing the type data
//! where the C frees it. The field offsets are those of the three structures
//! on the 64-bit targets libobs builds for.
//!
//! Cases marked "characterized, not endorsed" are odd but real (#1876) and
//! kept as the C has them. The tests after those pin the edges of each
//! function, and proptests cover any flags, capabilities, type, callbacks
//! and protocols.

use obs_core::module_source_rules::size_supported;
use obs_core::module_type_rules::{
    Broken, CAP_PASS_TEXTURE, CAP_SCALING, Callbacks, ENCODER_AUDIO, ENCODER_VIDEO, HAS_CREATE,
    HAS_DESTROY, HAS_ENCODE, HAS_ENCODE_TEXTURE, HAS_ENCODE_TEXTURE2, HAS_ENCODED_PACKET,
    HAS_GET_FRAME_SIZE, HAS_GET_NAME, HAS_GET_PROTOCOL, HAS_PROTOCOLS, HAS_RAW_AUDIO,
    HAS_RAW_AUDIO2, HAS_RAW_VIDEO, HAS_START, HAS_STOP, OUTPUT_AUDIO, OUTPUT_ENCODED,
    OUTPUT_MULTI_TRACK, OUTPUT_SERVICE, OUTPUT_VIDEO, encoder_check, output_check, protocol_listed,
    service_check,
};
use proptest::prelude::*;

const OUT_V: u32 = OUTPUT_VIDEO;
const OUT_A: u32 = OUTPUT_AUDIO;
const OUT_E: u32 = OUTPUT_ENCODED;
const OUT_S: u32 = OUTPUT_SERVICE;
const OUT_M: u32 = OUTPUT_MULTI_TRACK;
/// `OBS_OUTPUT_CAN_PAUSE`, `OBS_OUTPUT_MULTI_TRACK_VIDEO`.
const OUT_CAN_PAUSE: u32 = 1 << 5;
const OUT_MULTI_TRACK_VIDEO: u32 = 1 << 6;

/// `OBS_ENCODER_CAP_DEPRECATED | _DYN_BITRATE | _INTERNAL | _ROI |
/// _MULTITRACK_DYN_BITRATE`.
const CAP_OTHERS: u32 = 1 << 0 | 1 << 2 | 1 << 3 | 1 << 4 | 1 << 6;
const CAP_DYN_BITRATE: u32 = 1 << 2;

const ALL_CBS: u32 = (1 << 15) - 1;

/// The callbacks the rules look at, in the order the C checks them, with
/// their bits and the rule broken when one is missing.
const CBS: [(u32, Broken); 15] = [
    (HAS_GET_NAME, Broken::NoGetName),
    (HAS_CREATE, Broken::NoCreate),
    (HAS_DESTROY, Broken::NoDestroy),
    (HAS_START, Broken::NoStart),
    (HAS_STOP, Broken::NoStop),
    (HAS_PROTOCOLS, Broken::NoProtocols),
    (HAS_ENCODED_PACKET, Broken::NoEncodedPacket),
    (HAS_RAW_VIDEO, Broken::NoRawVideo),
    (HAS_RAW_AUDIO, Broken::NoRawAudio),
    (HAS_RAW_AUDIO2, Broken::NoRawAudio2),
    (HAS_ENCODE, Broken::NoEncode),
    (HAS_ENCODE_TEXTURE, Broken::NoEncodeTexture),
    (HAS_ENCODE_TEXTURE2, Broken::NoEncodeTexture),
    (HAS_GET_FRAME_SIZE, Broken::NoGetFrameSize),
    (HAS_GET_PROTOCOL, Broken::NoGetProtocol),
];

fn cb(bits: u32) -> Callbacks {
    Callbacks::from_bits(bits)
}

fn missing(bit: u32) -> Broken {
    CBS.iter()
        .find(|(b, _)| *b == bit)
        .map(|(_, broken)| *broken)
        .unwrap()
}

fn out(flags: u32, bits: u32) -> Result<(), Broken> {
    output_check(flags, cb(bits))
}

fn enc(encoder_type: u32, caps: u32, bits: u32) -> Result<(), Broken> {
    encoder_check(encoder_type, caps, cb(bits))
}

fn svc(bits: u32) -> Result<(), Broken> {
    service_check(cb(bits))
}

/// `output_needs` of the C test: every combination of the five flags the
/// rules read, and what it needs after get_name, create, destroy, start
/// and stop, in order.
const OUTPUT_NEEDS: [(u32, &[u32]); 32] = [
    (0, &[]),
    (OUT_V, &[HAS_RAW_VIDEO]),
    (OUT_A, &[HAS_RAW_AUDIO]),
    (OUT_V | OUT_A, &[HAS_RAW_VIDEO, HAS_RAW_AUDIO]),
    (OUT_E, &[HAS_ENCODED_PACKET]),
    (OUT_V | OUT_E, &[HAS_ENCODED_PACKET]),
    (OUT_A | OUT_E, &[HAS_ENCODED_PACKET]),
    (OUT_V | OUT_A | OUT_E, &[HAS_ENCODED_PACKET]),
    (OUT_S, &[HAS_PROTOCOLS]),
    (OUT_V | OUT_S, &[HAS_PROTOCOLS, HAS_RAW_VIDEO]),
    (OUT_A | OUT_S, &[HAS_PROTOCOLS, HAS_RAW_AUDIO]),
    (
        OUT_V | OUT_A | OUT_S,
        &[HAS_PROTOCOLS, HAS_RAW_VIDEO, HAS_RAW_AUDIO],
    ),
    (OUT_E | OUT_S, &[HAS_PROTOCOLS, HAS_ENCODED_PACKET]),
    (OUT_V | OUT_E | OUT_S, &[HAS_PROTOCOLS, HAS_ENCODED_PACKET]),
    (OUT_A | OUT_E | OUT_S, &[HAS_PROTOCOLS, HAS_ENCODED_PACKET]),
    (
        OUT_V | OUT_A | OUT_E | OUT_S,
        &[HAS_PROTOCOLS, HAS_ENCODED_PACKET],
    ),
    (OUT_M, &[]),
    (OUT_V | OUT_M, &[HAS_RAW_VIDEO]),
    (OUT_A | OUT_M, &[HAS_RAW_AUDIO2]),
    (OUT_V | OUT_A | OUT_M, &[HAS_RAW_VIDEO, HAS_RAW_AUDIO2]),
    (OUT_E | OUT_M, &[HAS_ENCODED_PACKET]),
    (OUT_V | OUT_E | OUT_M, &[HAS_ENCODED_PACKET]),
    (OUT_A | OUT_E | OUT_M, &[HAS_ENCODED_PACKET]),
    (OUT_V | OUT_A | OUT_E | OUT_M, &[HAS_ENCODED_PACKET]),
    (OUT_S | OUT_M, &[HAS_PROTOCOLS]),
    (OUT_V | OUT_S | OUT_M, &[HAS_PROTOCOLS, HAS_RAW_VIDEO]),
    (OUT_A | OUT_S | OUT_M, &[HAS_PROTOCOLS, HAS_RAW_AUDIO2]),
    (
        OUT_V | OUT_A | OUT_S | OUT_M,
        &[HAS_PROTOCOLS, HAS_RAW_VIDEO, HAS_RAW_AUDIO2],
    ),
    (OUT_E | OUT_S | OUT_M, &[HAS_PROTOCOLS, HAS_ENCODED_PACKET]),
    (
        OUT_V | OUT_E | OUT_S | OUT_M,
        &[HAS_PROTOCOLS, HAS_ENCODED_PACKET],
    ),
    (
        OUT_A | OUT_E | OUT_S | OUT_M,
        &[HAS_PROTOCOLS, HAS_ENCODED_PACKET],
    ),
    (
        OUT_V | OUT_A | OUT_E | OUT_S | OUT_M,
        &[HAS_PROTOCOLS, HAS_ENCODED_PACKET],
    ),
];

fn output_required(row: usize) -> Vec<u32> {
    let mut req = vec![HAS_GET_NAME, HAS_CREATE, HAS_DESTROY, HAS_START, HAS_STOP];
    req.extend_from_slice(OUTPUT_NEEDS[row].1);
    req
}

// ================================================================= steps

#[test]
fn output_flag_combinations() {
    for (row, &(flags, _)) in OUTPUT_NEEDS.iter().enumerate() {
        let req = output_required(row);
        assert_eq!(out(flags, ALL_CBS), Ok(()), "{flags:#x} all");
        assert_eq!(out(flags, 0), Err(Broken::NoGetName), "{flags:#x} none");
        let only = req.iter().fold(0, |a, b| a | b);
        assert_eq!(out(flags, only), Ok(()), "{flags:#x} only required");
        for (bit, broken) in CBS {
            let want = if req.contains(&bit) {
                Err(broken)
            } else {
                Ok(())
            };
            assert_eq!(
                out(flags, ALL_CBS & !bit),
                want,
                "{flags:#x} without {bit:#x}"
            );
        }
        let last = *req.last().unwrap();
        for &first in &req[..req.len() - 1] {
            assert_eq!(
                out(flags, only & !first & !last),
                Err(missing(first)),
                "{flags:#x} two missing"
            );
        }
        let mut cbs = only;
        for &bit in req.iter().rev() {
            cbs &= !bit;
            assert_eq!(
                out(flags, cbs),
                Err(missing(bit)),
                "{flags:#x} tail missing"
            );
        }
    }
}

#[test]
fn output_other_flags() {
    let others = [
        OUT_CAN_PAUSE,
        OUT_MULTI_TRACK_VIDEO,
        1 << 7,
        1 << 31,
        !(OUT_V | OUT_A | OUT_E | OUT_S | OUT_M),
    ];
    for (flags, _) in OUTPUT_NEEDS {
        for other in others {
            for bits in std::iter::once(ALL_CBS).chain(CBS.iter().map(|(b, _)| ALL_CBS & !b)) {
                assert_eq!(out(flags | other, bits), out(flags, bits));
            }
        }
    }
    let foreign = HAS_ENCODE
        | HAS_ENCODE_TEXTURE
        | HAS_ENCODE_TEXTURE2
        | HAS_GET_FRAME_SIZE
        | HAS_GET_PROTOCOL
        | 0xFFFF_8000;
    for (flags, _) in OUTPUT_NEEDS {
        assert_eq!(out(flags, ALL_CBS & !foreign), Ok(()));
    }
    assert_eq!(out(0, foreign), Err(Broken::NoGetName));

    let av = OUT_V | OUT_A;
    assert_eq!(out(av, ALL_CBS & !HAS_RAW_AUDIO), Err(Broken::NoRawAudio));
    assert_eq!(
        out(av | OUT_M | OUT_MULTI_TRACK_VIDEO, ALL_CBS & !HAS_RAW_AUDIO),
        Ok(())
    );
    assert_eq!(
        out(av | OUT_E | OUT_S, ALL_CBS & !HAS_PROTOCOLS),
        Err(Broken::NoProtocols)
    );
    // Multi-track alone needs no audio callback (characterized, not
    // endorsed).
    assert_eq!(
        out(OUT_M, ALL_CBS & !HAS_RAW_AUDIO & !HAS_RAW_AUDIO2),
        Ok(())
    );
}

const ENCODER_TYPES: [u32; 5] = [ENCODER_AUDIO, ENCODER_VIDEO, 2, 0x8000_0000, u32::MAX];

#[test]
fn encoder_texture_scaling() {
    let pass = CAP_PASS_TEXTURE;
    let scale = CAP_SCALING;
    for t in ENCODER_TYPES {
        for caps in [pass | scale, pass | scale | CAP_OTHERS, u32::MAX] {
            assert_eq!(enc(t, caps, ALL_CBS), Err(Broken::TextureScaling));
            assert_eq!(enc(t, caps, 0), Err(Broken::TextureScaling));
        }
        assert_eq!(enc(t, scale, ALL_CBS), Ok(()));
        assert_eq!(enc(t, pass, ALL_CBS), Ok(()));
        assert_eq!(enc(t, !scale, ALL_CBS), Ok(()));
        assert_eq!(enc(t, !pass, ALL_CBS), Ok(()));
    }
}

#[test]
fn encoder_callbacks() {
    let pass = CAP_PASS_TEXTURE;
    for t in ENCODER_TYPES {
        let audio = t == ENCODER_AUDIO;
        for caps in [0, CAP_SCALING, CAP_OTHERS, pass, pass | CAP_OTHERS] {
            let texture = caps & pass != 0;
            let mut req = vec![HAS_GET_NAME, HAS_CREATE, HAS_DESTROY];
            if !texture {
                req.push(HAS_ENCODE);
            }
            if audio {
                req.push(HAS_GET_FRAME_SIZE);
            }
            assert_eq!(enc(t, caps, ALL_CBS), Ok(()));
            assert_eq!(enc(t, caps, 0), Err(Broken::NoGetName));
            for (bit, broken) in CBS {
                let want = if req.contains(&bit) {
                    Err(broken)
                } else {
                    Ok(())
                };
                assert_eq!(enc(t, caps, ALL_CBS & !bit), want, "{t} {caps:#x} {bit:#x}");
            }
            let base = HAS_GET_NAME
                | HAS_CREATE
                | HAS_DESTROY
                | if audio { HAS_GET_FRAME_SIZE } else { 0 };
            let tex = HAS_ENCODE_TEXTURE | HAS_ENCODE_TEXTURE2;
            if texture {
                assert_eq!(enc(t, caps, base | HAS_ENCODE_TEXTURE), Ok(()));
                assert_eq!(enc(t, caps, base | HAS_ENCODE_TEXTURE2), Ok(()));
                assert_eq!(
                    enc(t, caps, base | HAS_ENCODE),
                    Err(Broken::NoEncodeTexture)
                );
                assert_eq!(enc(t, caps, base), Err(Broken::NoEncodeTexture));
            } else {
                assert_eq!(enc(t, caps, base | HAS_ENCODE), Ok(()));
                assert_eq!(enc(t, caps, base | tex), Err(Broken::NoEncode));
            }
            if audio {
                let want = if texture {
                    Broken::NoEncodeTexture
                } else {
                    Broken::NoEncode
                };
                assert_eq!(
                    enc(t, caps, HAS_GET_NAME | HAS_CREATE | HAS_DESTROY),
                    Err(want)
                );
            }
        }
    }
}

#[test]
fn service_callbacks() {
    let req = [HAS_GET_NAME, HAS_CREATE, HAS_DESTROY, HAS_GET_PROTOCOL];
    assert_eq!(svc(ALL_CBS), Ok(()));
    assert_eq!(svc(u32::MAX), Ok(()));
    assert_eq!(svc(0), Err(Broken::NoGetName));
    let only = req.iter().fold(0, |a, b| a | b);
    assert_eq!(svc(only), Ok(()));
    for (bit, broken) in CBS {
        let want = if req.contains(&bit) {
            Err(broken)
        } else {
            Ok(())
        };
        assert_eq!(svc(ALL_CBS & !bit), want);
    }
    let mut cbs = only;
    for &bit in req.iter().rev() {
        cbs &= !bit;
        assert_eq!(svc(cbs), Err(missing(bit)));
    }
}

fn listed(protocol: &str, list: &[&str]) -> bool {
    protocol_listed(protocol.as_bytes(), list.iter().map(|p| p.as_bytes()))
}

#[test]
fn protocol_listed_cases() {
    let list = ["RTMP", "RTMPS", "SRT", "", "rtmp", " RIST", "RTMP"];
    assert!(!listed("RTMP", &[]));
    assert!(!listed("", &[]));
    for p in ["RTMP", "RTMPS", "SRT", "rtmp", " RIST", ""] {
        assert!(listed(p, &list), "{p}");
    }
    assert!(!listed("", &list[..3]));
    for p in ["RTM", "RTMPSX", "Rtmp", "RIST", "SRT "] {
        assert!(!listed(p, &list), "{p}");
    }
    assert!(!listed("SRT", &list[..2]));
    assert!(listed("SRT", &list[..3]));
    assert!(!listed("RTMP", &list[1..6]));
    assert!(listed("RTMP", &list[1..7]));
    let u = [b"\xc3\xa9t\xc3\xa9".as_slice()];
    assert!(protocol_listed(b"\xc3\xa9t\xc3\xa9", u));
    assert!(!protocol_listed(b"\xc3\x89T\xc3\x89", u));
}

// =============================================================== callers

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Level {
    Warning,
    Error,
}
const W: Level = Level::Warning;
const E: Level = Level::Error;

const PTR: usize = 8;

/// `offsetof(struct obs_output_info, ...)` on 64-bit targets.
mod output_off {
    pub const FLAGS: usize = 8;
    pub const GET_NAME: usize = 16;
    pub const CREATE: usize = 24;
    pub const DESTROY: usize = 32;
    pub const START: usize = 40;
    pub const STOP: usize = 48;
    pub const RAW_VIDEO: usize = 56;
    pub const RAW_AUDIO: usize = 64;
    pub const ENCODED_PACKET: usize = 72;
    pub const TYPE_DATA: usize = 128;
    pub const FREE_TYPE_DATA: usize = 136;
    pub const RAW_AUDIO2: usize = 176;
    pub const PROTOCOLS: usize = 184;
    /// `sizeof(struct obs_output_info)`.
    pub const SIZE: usize = 192;
}

/// `offsetof(struct obs_encoder_info, ...)` on 64-bit targets.
mod encoder_off {
    pub const TYPE: usize = 8;
    pub const GET_NAME: usize = 24;
    pub const CREATE: usize = 32;
    pub const DESTROY: usize = 40;
    pub const ENCODE: usize = 48;
    pub const GET_FRAME_SIZE: usize = 56;
    pub const TYPE_DATA: usize = 120;
    pub const FREE_TYPE_DATA: usize = 128;
    pub const CAPS: usize = 136;
    pub const ENCODE_TEXTURE: usize = 160;
    pub const ENCODE_TEXTURE2: usize = 168;
    /// `sizeof(struct obs_encoder_info)`.
    pub const SIZE: usize = 192;
}

/// `offsetof(struct obs_service_info, ...)` on 64-bit targets.
mod service_off {
    pub const GET_NAME: usize = 8;
    pub const CREATE: usize = 16;
    pub const DESTROY: usize = 24;
    pub const TYPE_DATA: usize = 128;
    pub const FREE_TYPE_DATA: usize = 136;
    pub const GET_PROTOCOL: usize = 184;
    /// `sizeof(struct obs_service_info)`.
    pub const SIZE: usize = 216;
}

/// `HAS_VAL()`: set, and whole inside the size.
fn inside(set: bool, offset: usize, len: usize, size: usize) -> bool {
    set && offset + len <= size
}

/// A field as `REGISTER_OBS_DEF()` copies it: whole inside the size, or
/// zero. The copy ends at the last field wholly within the size, so a field
/// the size cuts is not partly copied (#2203).
fn copied<T: Default>(value: T, offset: usize, len: usize, size: usize) -> T {
    if offset + len <= size {
        value
    } else {
        T::default()
    }
}

/// The fields the registration reads; a callback is set or NULL.
#[derive(Debug, Clone)]
struct Info {
    id: &'static str,
    /// Outputs: flags; encoders: caps.
    flags: u32,
    /// Encoders.
    encoder_type: u32,
    protocols: Option<&'static str>,
    cbs: u32,
    type_data: bool,
    free_type_data: bool,
}

const OUTPUT_CBS: u32 = HAS_GET_NAME
    | HAS_CREATE
    | HAS_DESTROY
    | HAS_START
    | HAS_STOP
    | HAS_PROTOCOLS
    | HAS_ENCODED_PACKET
    | HAS_RAW_VIDEO
    | HAS_RAW_AUDIO
    | HAS_RAW_AUDIO2;
const ENCODER_CBS: u32 = HAS_GET_NAME
    | HAS_CREATE
    | HAS_DESTROY
    | HAS_ENCODE
    | HAS_GET_FRAME_SIZE
    | HAS_ENCODE_TEXTURE
    | HAS_ENCODE_TEXTURE2;
const SERVICE_CBS: u32 = HAS_GET_NAME | HAS_CREATE | HAS_DESTROY | HAS_GET_PROTOCOL;

/// `full_output()` of the C test: `protocols` NULL for `None`.
fn full_output(id: &'static str, flags: u32, protocols: Option<&'static str>) -> Info {
    Info {
        id,
        flags,
        encoder_type: 0,
        protocols,
        cbs: OUTPUT_CBS
            & if protocols.is_some() {
                !0
            } else {
                !HAS_PROTOCOLS
            },
        type_data: true,
        free_type_data: true,
    }
}

fn full_encoder(id: &'static str, encoder_type: u32, caps: u32) -> Info {
    Info {
        id,
        flags: caps,
        encoder_type,
        protocols: None,
        cbs: ENCODER_CBS,
        type_data: true,
        free_type_data: true,
    }
}

fn full_service(id: &'static str) -> Info {
    Info {
        id,
        flags: 0,
        encoder_type: 0,
        protocols: None,
        cbs: SERVICE_CBS,
        type_data: true,
        free_type_data: true,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Output,
    Encoder,
    Service,
}

#[derive(Debug, Clone)]
struct Entry {
    id: &'static str,
    flags: u32,
    encoder_type: u32,
    type_data: bool,
}

#[derive(Default)]
struct Registry {
    outputs: Vec<Entry>,
    encoders: Vec<Entry>,
    services: Vec<Entry>,
    protocols: Vec<Vec<u8>>,
    logged: Vec<(Level, String)>,
    frees: usize,
}

fn required(val: &str, id: &str, func: &str) -> String {
    format!("Required value '{val}' for '{id}' not found.  {func} failed.")
}

fn out_required(val: &str, id: &str) -> String {
    required(val, id, "obs_register_output")
}

fn enc_required(val: &str, id: &str) -> String {
    required(val, id, "obs_register_encoder")
}

fn svc_required(val: &str, id: &str) -> String {
    required(val, id, "obs_register_service")
}

fn neither(id: &str) -> String {
    format!(
        "Neither 'encode_texture' nor 'encode_texture2' for '{id}' found.  obs_register_encoder failed."
    )
}

fn texture_scaling(id: &str) -> String {
    format!(
        "obs_register_encoder: Texture encoders cannot self-scale. Encoder id '{id}' not registered."
    )
}

fn too_big(structure: &str, size: usize, full: usize) -> String {
    format!(
        "Tried to register {structure} with size {size} which is more than libobs currently supports ({full})"
    )
}

fn name_of(broken: Broken) -> &'static str {
    match broken {
        Broken::TextureScaling => "",
        Broken::NoGetName => "get_name",
        Broken::NoCreate => "create",
        Broken::NoDestroy => "destroy",
        Broken::NoStart => "start",
        Broken::NoStop => "stop",
        Broken::NoProtocols => "protocols",
        Broken::NoEncodedPacket => "encoded_packet",
        Broken::NoRawVideo => "raw_video",
        Broken::NoRawAudio => "raw_audio",
        Broken::NoRawAudio2 => "raw_audio2",
        Broken::NoEncodeTexture => "",
        Broken::NoEncode => "encode",
        Broken::NoGetFrameSize => "get_frame_size",
        Broken::NoGetProtocol => "get_protocol",
    }
}

impl Registry {
    fn list(&mut self, kind: Kind) -> &mut Vec<Entry> {
        match kind {
            Kind::Output => &mut self.outputs,
            Kind::Encoder => &mut self.encoders,
            Kind::Service => &mut self.services,
        }
    }

    fn find(&self, kind: Kind, id: &str) -> Option<&Entry> {
        let list = match kind {
            Kind::Output => &self.outputs,
            Kind::Encoder => &self.encoders,
            Kind::Service => &self.services,
        };
        list.iter().find(|e| e.id == id)
    }

    fn count(&self, kind: Kind) -> usize {
        match kind {
            Kind::Output => self.outputs.len(),
            Kind::Encoder => self.encoders.len(),
            Kind::Service => self.services.len(),
        }
    }

    /// The callbacks `HAS_VAL()` finds in `info` cut at `size`.
    fn callbacks(kind: Kind, info: &Info, size: usize) -> u32 {
        let fields: &[(u32, usize)] = match kind {
            Kind::Output => &[
                (HAS_GET_NAME, output_off::GET_NAME),
                (HAS_CREATE, output_off::CREATE),
                (HAS_DESTROY, output_off::DESTROY),
                (HAS_START, output_off::START),
                (HAS_STOP, output_off::STOP),
                (HAS_PROTOCOLS, output_off::PROTOCOLS),
                (HAS_ENCODED_PACKET, output_off::ENCODED_PACKET),
                (HAS_RAW_VIDEO, output_off::RAW_VIDEO),
                (HAS_RAW_AUDIO, output_off::RAW_AUDIO),
                (HAS_RAW_AUDIO2, output_off::RAW_AUDIO2),
            ],
            Kind::Encoder => &[
                (HAS_GET_NAME, encoder_off::GET_NAME),
                (HAS_CREATE, encoder_off::CREATE),
                (HAS_DESTROY, encoder_off::DESTROY),
                (HAS_ENCODE, encoder_off::ENCODE),
                (HAS_ENCODE_TEXTURE, encoder_off::ENCODE_TEXTURE),
                (HAS_ENCODE_TEXTURE2, encoder_off::ENCODE_TEXTURE2),
                (HAS_GET_FRAME_SIZE, encoder_off::GET_FRAME_SIZE),
            ],
            Kind::Service => &[
                (HAS_GET_NAME, service_off::GET_NAME),
                (HAS_CREATE, service_off::CREATE),
                (HAS_DESTROY, service_off::DESTROY),
                (HAS_GET_PROTOCOL, service_off::GET_PROTOCOL),
            ],
        };
        let mut bits = 0;
        for &(bit, offset) in fields {
            if inside(info.cbs & bit != 0, offset, PTR, size) {
                bits |= bit;
            }
        }
        bits
    }

    /// `HANDLE_ERROR()`: nothing for size 0, else free the type data when
    /// the part inside the size holds both.
    fn handle_error(&mut self, kind: Kind, info: &Info, size: usize) {
        if size == 0 {
            return;
        }
        let (td, free) = match kind {
            Kind::Output => (output_off::TYPE_DATA, output_off::FREE_TYPE_DATA),
            Kind::Encoder => (encoder_off::TYPE_DATA, encoder_off::FREE_TYPE_DATA),
            Kind::Service => (service_off::TYPE_DATA, service_off::FREE_TYPE_DATA),
        };
        if copied(info.type_data, td, PTR, size) && copied(info.free_type_data, free, PTR, size) {
            self.frees += 1;
        }
    }

    fn register(&mut self, kind: Kind, info: &Info, size: usize) {
        self.logged.clear();
        self.frees = 0;
        if self.try_register(kind, info, size).is_err() {
            self.handle_error(kind, info, size);
        }
    }

    fn try_register(&mut self, kind: Kind, info: &Info, size: usize) -> Result<(), ()> {
        let (prefix, what) = match kind {
            Kind::Output => ("obs_register_output", "Output"),
            Kind::Encoder => ("obs_register_encoder", "Encoder"),
            Kind::Service => ("obs_register_service", "Service"),
        };
        if self.find(kind, info.id).is_some() {
            self.logged.push((
                W,
                format!(
                    "{prefix}: {what} id '{}' already exists!  Duplicate library?",
                    info.id
                ),
            ));
            return Err(());
        }

        let cbs = cb(Self::callbacks(kind, info, size));
        let result = match kind {
            Kind::Output => output_check(info.flags, cbs),
            Kind::Encoder => encoder_check(info.encoder_type, info.flags, cbs),
            Kind::Service => service_check(cbs),
        };
        if let Err(broken) = result {
            let msg = match broken {
                Broken::TextureScaling => (W, texture_scaling(info.id)),
                Broken::NoEncodeTexture => (E, neither(info.id)),
                other => (E, required(name_of(other), info.id, prefix)),
            };
            self.logged.push(msg);
            return Err(());
        }

        // REGISTER_OBS_DEF()
        let (structure, full) = match kind {
            Kind::Output => ("obs_output_info", output_off::SIZE),
            Kind::Encoder => ("obs_encoder_info", encoder_off::SIZE),
            Kind::Service => ("obs_service_info", service_off::SIZE),
        };
        assert_ne!(size, 0, "unreachable: get_name is missing at size 0");
        if !size_supported(size, full) {
            self.logged.push((E, too_big(structure, size, full)));
            return Err(());
        }
        let entry = match kind {
            Kind::Output => Entry {
                id: info.id,
                flags: copied(info.flags, output_off::FLAGS, 4, size),
                encoder_type: 0,
                type_data: copied(info.type_data, output_off::TYPE_DATA, PTR, size),
            },
            Kind::Encoder => Entry {
                id: info.id,
                flags: copied(info.flags, encoder_off::CAPS, 4, size),
                encoder_type: copied(info.encoder_type, encoder_off::TYPE, 4, size),
                type_data: copied(info.type_data, encoder_off::TYPE_DATA, PTR, size),
            },
            Kind::Service => Entry {
                id: info.id,
                flags: 0,
                encoder_type: 0,
                type_data: copied(info.type_data, service_off::TYPE_DATA, PTR, size),
            },
        };
        self.list(kind).push(entry);

        if kind == Kind::Output && info.flags & OUT_S != 0 {
            // strlist_split(info->protocols, ';', false)
            let protocols = info.protocols.unwrap_or_default();
            for protocol in protocols.split(';').filter(|p| !p.is_empty()) {
                if protocol_listed(
                    protocol.as_bytes(),
                    self.protocols.iter().map(Vec::as_slice),
                ) {
                    continue;
                }
                self.protocols.push(protocol.as_bytes().to_vec());
            }
        }
        Ok(())
    }

    fn expect_logged(&self, expected: &[(Level, &str)]) {
        let got: Vec<(Level, &str)> = self.logged.iter().map(|(l, m)| (*l, m.as_str())).collect();
        assert_eq!(got, expected);
    }

    fn expect_freed(&self, freed: bool) {
        assert_eq!(self.frees, usize::from(freed));
    }

    fn expect_output(&self, id: &str, flags: u32) {
        let entry = self.find(Kind::Output, id).expect("registered");
        assert_eq!(entry.flags, flags);
        assert!(entry.type_data);
    }

    fn expect_no(&self, kind: Kind, id: &str, types: usize, freed: bool) {
        assert!(self.find(kind, id).is_none());
        assert_eq!(self.count(kind), types);
        self.expect_freed(freed);
    }

    fn expect_encoder(&self, id: &str, encoder_type: u32, caps: u32) {
        let entry = self.find(Kind::Encoder, id).expect("registered");
        assert_eq!(entry.encoder_type, encoder_type);
        assert_eq!(entry.flags, caps);
        assert!(entry.type_data);
    }

    fn expect_service(&self, id: &str) {
        let entry = self.find(Kind::Service, id).expect("registered");
        assert!(entry.type_data);
    }

    fn protocols_from(&self, from: usize) -> String {
        let all: Vec<String> = self
            .protocols
            .iter()
            .skip(from)
            .map(|p| String::from_utf8_lossy(p).into_owned())
            .collect();
        all.join(",")
    }

    fn protocol_registered(&self, protocol: &str) -> bool {
        self.protocols.iter().any(|p| p == protocol.as_bytes())
    }
}

fn reg_output(r: &mut Registry, info: &Info, size: usize) {
    r.register(Kind::Output, info, size);
}

fn reg_encoder(r: &mut Registry, info: &Info, size: usize) {
    r.register(Kind::Encoder, info, size);
}

fn reg_service(r: &mut Registry, info: &Info, size: usize) {
    r.register(Kind::Service, info, size);
}

fn ids() -> &'static [&'static str; 32] {
    &[
        "mtr_out_0",
        "mtr_out_1",
        "mtr_out_2",
        "mtr_out_3",
        "mtr_out_4",
        "mtr_out_5",
        "mtr_out_6",
        "mtr_out_7",
        "mtr_out_8",
        "mtr_out_9",
        "mtr_out_10",
        "mtr_out_11",
        "mtr_out_12",
        "mtr_out_13",
        "mtr_out_14",
        "mtr_out_15",
        "mtr_out_16",
        "mtr_out_17",
        "mtr_out_18",
        "mtr_out_19",
        "mtr_out_20",
        "mtr_out_21",
        "mtr_out_22",
        "mtr_out_23",
        "mtr_out_24",
        "mtr_out_25",
        "mtr_out_26",
        "mtr_out_27",
        "mtr_out_28",
        "mtr_out_29",
        "mtr_out_30",
        "mtr_out_31",
    ]
}

#[test]
fn output_register() {
    let mut r = Registry::default();
    for (&id, &(flags, _)) in ids().iter().zip(OUTPUT_NEEDS.iter()) {
        reg_output(
            &mut r,
            &full_output(id, flags, Some("MTR_ALL")),
            output_off::SIZE,
        );
        r.expect_logged(&[]);
        r.expect_freed(false);
        r.expect_output(id, flags);
    }

    let mut info = full_output("mtr_out_min_av", OUT_V | OUT_A, None);
    info.cbs &= !HAS_ENCODED_PACKET & !HAS_RAW_AUDIO2;
    reg_output(&mut r, &info, output_off::SIZE);
    r.expect_logged(&[]);
    r.expect_output("mtr_out_min_av", OUT_V | OUT_A);

    let mut info = full_output("mtr_out_min_enc", OUT_V | OUT_A | OUT_E, None);
    info.cbs &= !HAS_RAW_VIDEO & !HAS_RAW_AUDIO & !HAS_RAW_AUDIO2;
    reg_output(&mut r, &info, output_off::SIZE);
    r.expect_logged(&[]);
    r.expect_output("mtr_out_min_enc", OUT_V | OUT_A | OUT_E);

    let mut info = full_output("mtr_out_min_mt", OUT_A | OUT_M, None);
    info.cbs &= !HAS_RAW_AUDIO;
    reg_output(&mut r, &info, output_off::SIZE);
    r.expect_logged(&[]);
    r.expect_output("mtr_out_min_mt", OUT_A | OUT_M);
}

#[test]
fn output_required_callbacks() {
    let mut r = Registry::default();
    let types = r.count(Kind::Output);
    let every = OUT_V | OUT_A | OUT_S;
    let cases: [(u32, u32, &str, &str); 10] = [
        (HAS_GET_NAME, every, "mtr_out_no_get_name", "get_name"),
        (HAS_CREATE, every, "mtr_out_no_create", "create"),
        (HAS_DESTROY, every, "mtr_out_no_destroy", "destroy"),
        (HAS_START, every, "mtr_out_no_start", "start"),
        (HAS_STOP, every, "mtr_out_no_stop", "stop"),
        (HAS_PROTOCOLS, every, "mtr_out_no_protocols", "protocols"),
        (
            HAS_ENCODED_PACKET,
            OUT_V | OUT_A | OUT_E,
            "mtr_out_no_encoded",
            "encoded_packet",
        ),
        (
            HAS_RAW_VIDEO,
            OUT_V | OUT_A,
            "mtr_out_no_raw_video",
            "raw_video",
        ),
        (
            HAS_RAW_AUDIO,
            OUT_V | OUT_A,
            "mtr_out_no_raw_audio",
            "raw_audio",
        ),
        (
            HAS_RAW_AUDIO2,
            OUT_V | OUT_A | OUT_M,
            "mtr_out_no_raw_audio2",
            "raw_audio2",
        ),
    ];
    for (bit, flags, id, val) in cases {
        let mut info = full_output(id, flags, Some("MTR_X"));
        info.cbs &= !bit;
        reg_output(&mut r, &info, output_off::SIZE);
        r.expect_logged(&[(E, &out_required(val, id))]);
        r.expect_no(Kind::Output, id, types, true);
    }

    let mut info = full_output("mtr_out_no_many", every | OUT_E, None);
    info.cbs &= !HAS_STOP & !HAS_ENCODED_PACKET;
    reg_output(&mut r, &info, output_off::SIZE);
    r.expect_logged(&[(E, &out_required("stop", "mtr_out_no_many"))]);
    r.expect_no(Kind::Output, "mtr_out_no_many", types, true);

    let protocols = r.protocols.len();
    let mut info = full_output("mtr_out_failed_service", OUT_S, Some("MTR_NEVER"));
    info.cbs &= !HAS_START;
    reg_output(&mut r, &info, output_off::SIZE);
    r.expect_logged(&[(E, &out_required("start", "mtr_out_failed_service"))]);
    assert_eq!(r.protocols.len(), protocols);
    assert!(!r.protocol_registered("MTR_NEVER"));

    let mut info = full_output("mtr_out_nofree", 0, None);
    info.cbs &= !HAS_GET_NAME;
    info.free_type_data = false;
    reg_output(&mut r, &info, output_off::SIZE);
    r.expect_no(Kind::Output, "mtr_out_nofree", types, false);
    info.free_type_data = true;
    info.type_data = false;
    reg_output(&mut r, &info, output_off::SIZE);
    r.expect_no(Kind::Output, "mtr_out_nofree", types, false);
}

fn dup(prefix: &str, what: &str, id: &str) -> String {
    format!("{prefix}: {what} id '{id}' already exists!  Duplicate library?")
}

#[test]
fn output_duplicate() {
    let mut r = Registry::default();
    reg_output(
        &mut r,
        &full_output("mtr_out_dup", OUT_V, None),
        output_off::SIZE,
    );
    r.expect_logged(&[]);
    let types = r.count(Kind::Output);

    let mut info = full_output("mtr_out_dup", OUT_A, None);
    info.cbs &= !HAS_GET_NAME;
    reg_output(&mut r, &info, output_off::SIZE + 8);
    let msg = dup("obs_register_output", "Output", "mtr_out_dup");
    r.expect_logged(&[(W, &msg)]);
    assert_eq!(r.count(Kind::Output), types);
    r.expect_freed(true);
    r.expect_output("mtr_out_dup", OUT_V);

    reg_output(&mut r, &info, 0);
    r.expect_logged(&[(W, &msg)]);
    r.expect_freed(false);
}

#[test]
fn output_size() {
    let mut r = Registry::default();
    let mut types = r.count(Kind::Output);
    let full = output_off::SIZE;

    reg_output(&mut r, &full_output("mtr_out_size0", 0, None), 0);
    r.expect_logged(&[(E, &out_required("get_name", "mtr_out_size0"))]);
    r.expect_no(Kind::Output, "mtr_out_size0", types, false);

    let info = full_output(
        "mtr_out_size_at",
        OUT_V | OUT_A | OUT_S,
        Some("MTR_SIZE_AT"),
    );
    reg_output(&mut r, &info, full);
    r.expect_logged(&[]);
    r.expect_output("mtr_out_size_at", OUT_V | OUT_A | OUT_S);
    assert!(r.protocol_registered("MTR_SIZE_AT"));
    types += 1;

    let mut info = full_output("mtr_out_size_above", OUT_V | OUT_A, None);
    reg_output(&mut r, &info, full + 1);
    r.expect_logged(&[(E, &too_big("obs_output_info", full + 1, full))]);
    r.expect_no(Kind::Output, "mtr_out_size_above", types, true);

    info.cbs &= !HAS_GET_NAME;
    reg_output(&mut r, &info, full + 8);
    r.expect_logged(&[(E, &out_required("get_name", "mtr_out_size_above"))]);
    r.expect_no(Kind::Output, "mtr_out_size_above", types, true);

    let protocols_end = output_off::PROTOCOLS + PTR;
    let info = full_output("mtr_out_size_cut", OUT_S, Some("MTR_CUT"));
    reg_output(&mut r, &info, protocols_end - 1);
    r.expect_logged(&[(E, &out_required("protocols", "mtr_out_size_cut"))]);
    r.expect_no(Kind::Output, "mtr_out_size_cut", types, true);
    assert!(!r.protocol_registered("MTR_CUT"));

    let raw_audio2_end = output_off::RAW_AUDIO2 + PTR;
    let info = full_output("mtr_out_size_cut2", OUT_A | OUT_M, None);
    reg_output(&mut r, &info, raw_audio2_end - 1);
    r.expect_logged(&[(E, &out_required("raw_audio2", "mtr_out_size_cut2"))]);
    r.expect_no(Kind::Output, "mtr_out_size_cut2", types, true);

    reg_output(&mut r, &info, raw_audio2_end);
    r.expect_logged(&[]);
    r.expect_output("mtr_out_size_cut2", OUT_A | OUT_M);

    let protocols_count = r.protocols.len();
    let info = full_output(
        "mtr_out_size_noservice",
        OUT_V | OUT_A,
        Some("MTR_NOSERVICE"),
    );
    reg_output(&mut r, &info, output_off::PROTOCOLS);
    r.expect_logged(&[]);
    r.expect_output("mtr_out_size_noservice", OUT_V | OUT_A);
    assert_eq!(r.protocols.len(), protocols_count);

    let info = full_output("mtr_out_size_nofree", OUT_S, Some("MTR_X"));
    reg_output(&mut r, &info, output_off::FREE_TYPE_DATA);
    r.expect_logged(&[(E, &out_required("protocols", "mtr_out_size_nofree"))]);
    r.expect_freed(false);
    reg_output(&mut r, &info, output_off::FREE_TYPE_DATA + PTR);
    r.expect_logged(&[(E, &out_required("protocols", "mtr_out_size_nofree"))]);
    r.expect_freed(true);
}

#[test]
fn output_protocols() {
    let mut r = Registry::default();
    let from = r.protocols.len();
    let full = output_off::SIZE;

    reg_output(
        &mut r,
        &full_output("mtr_out_proto1", OUT_S, Some("MTR_P1;MTR_P2")),
        full,
    );
    r.expect_logged(&[]);
    assert_eq!(r.protocols_from(from), "MTR_P1,MTR_P2");

    let info = full_output(
        "mtr_out_proto2",
        OUT_S | OUT_E,
        Some(";MTR_P2;MTR_P3;;MTR_P3;MTR_P1;"),
    );
    reg_output(&mut r, &info, full);
    r.expect_logged(&[]);
    assert_eq!(r.protocols_from(from), "MTR_P1,MTR_P2,MTR_P3");

    let info = full_output("mtr_out_proto3", OUT_S, Some("mtr_p1; MTR_P2;MTR_P2 "));
    reg_output(&mut r, &info, full);
    assert_eq!(
        r.protocols_from(from),
        "MTR_P1,MTR_P2,MTR_P3,mtr_p1, MTR_P2,MTR_P2 "
    );
    assert!(r.protocol_registered("mtr_p1"));
    assert!(r.protocol_registered(" MTR_P2"));

    let n = r.protocols.len();
    let info = full_output("mtr_out_proto4", OUT_V | OUT_A | OUT_E, Some("MTR_P4"));
    reg_output(&mut r, &info, full);
    r.expect_logged(&[]);
    assert_eq!(r.protocols.len(), n);
    assert!(!r.protocol_registered("MTR_P4"));

    reg_output(
        &mut r,
        &full_output("mtr_out_proto5", OUT_S, Some("")),
        full,
    );
    r.expect_logged(&[]);
    r.expect_output("mtr_out_proto5", OUT_S);
    reg_output(
        &mut r,
        &full_output("mtr_out_proto6", OUT_S, Some(";;")),
        full,
    );
    r.expect_logged(&[]);
    assert_eq!(r.protocols.len(), n);

    reg_output(
        &mut r,
        &full_output("mtr_out_proto1", OUT_S, Some("MTR_P9")),
        full,
    );
    r.expect_logged(&[(W, &dup("obs_register_output", "Output", "mtr_out_proto1"))]);
    assert_eq!(r.protocols.len(), n);
}

#[test]
fn encoder_register() {
    let mut r = Registry::default();
    let full = encoder_off::SIZE;

    let mut info = full_encoder("mtr_enc_video", ENCODER_VIDEO, 0);
    info.cbs &= !HAS_GET_FRAME_SIZE & !HAS_ENCODE_TEXTURE & !HAS_ENCODE_TEXTURE2;
    reg_encoder(&mut r, &info, full);
    r.expect_logged(&[]);
    r.expect_freed(false);
    r.expect_encoder("mtr_enc_video", ENCODER_VIDEO, 0);

    let mut info = full_encoder("mtr_enc_audio", ENCODER_AUDIO, CAP_DYN_BITRATE);
    info.cbs &= !HAS_ENCODE_TEXTURE & !HAS_ENCODE_TEXTURE2;
    reg_encoder(&mut r, &info, full);
    r.expect_logged(&[]);
    r.expect_encoder("mtr_enc_audio", ENCODER_AUDIO, CAP_DYN_BITRATE);

    let mut info = full_encoder("mtr_enc_tex1", ENCODER_VIDEO, CAP_PASS_TEXTURE);
    info.cbs &= !HAS_ENCODE & !HAS_ENCODE_TEXTURE2;
    reg_encoder(&mut r, &info, full);
    r.expect_logged(&[]);
    r.expect_encoder("mtr_enc_tex1", ENCODER_VIDEO, CAP_PASS_TEXTURE);

    let caps = CAP_PASS_TEXTURE | CAP_OTHERS;
    let mut info = full_encoder("mtr_enc_tex2", ENCODER_VIDEO, caps);
    info.cbs &= !HAS_ENCODE & !HAS_ENCODE_TEXTURE;
    reg_encoder(&mut r, &info, full);
    r.expect_logged(&[]);
    r.expect_encoder("mtr_enc_tex2", ENCODER_VIDEO, caps);

    reg_encoder(
        &mut r,
        &full_encoder("mtr_enc_scale", ENCODER_VIDEO, CAP_SCALING),
        full,
    );
    r.expect_logged(&[]);
    r.expect_encoder("mtr_enc_scale", ENCODER_VIDEO, CAP_SCALING);

    // A type that is neither audio nor video is treated as video
    // (characterized, not endorsed).
    let mut info = full_encoder("mtr_enc_type7", 7, 0);
    info.cbs &= !HAS_GET_FRAME_SIZE;
    reg_encoder(&mut r, &info, full);
    r.expect_logged(&[]);
    r.expect_encoder("mtr_enc_type7", 7, 0);
}

#[test]
fn encoder_required_callbacks() {
    let mut r = Registry::default();
    let types = r.count(Kind::Encoder);
    let cases: [(u32, u32, &str, &str); 5] = [
        (
            HAS_GET_NAME,
            ENCODER_AUDIO,
            "mtr_enc_no_get_name",
            "get_name",
        ),
        (HAS_CREATE, ENCODER_AUDIO, "mtr_enc_no_create", "create"),
        (HAS_DESTROY, ENCODER_AUDIO, "mtr_enc_no_destroy", "destroy"),
        (HAS_ENCODE, ENCODER_VIDEO, "mtr_enc_no_encode", "encode"),
        (
            HAS_GET_FRAME_SIZE,
            ENCODER_AUDIO,
            "mtr_enc_no_frame_size",
            "get_frame_size",
        ),
    ];
    for (bit, t, id, val) in cases {
        let mut info = full_encoder(id, t, 0);
        info.cbs &= !bit;
        reg_encoder(&mut r, &info, encoder_off::SIZE);
        r.expect_logged(&[(E, &enc_required(val, id))]);
        r.expect_no(Kind::Encoder, id, types, true);
    }

    let mut info = full_encoder("mtr_enc_no_tex", ENCODER_VIDEO, CAP_PASS_TEXTURE);
    info.cbs &= !HAS_ENCODE_TEXTURE & !HAS_ENCODE_TEXTURE2;
    reg_encoder(&mut r, &info, encoder_off::SIZE);
    r.expect_logged(&[(E, &neither("mtr_enc_no_tex"))]);
    r.expect_no(Kind::Encoder, "mtr_enc_no_tex", types, true);

    let mut info = full_encoder("mtr_enc_no_both", ENCODER_AUDIO, 0);
    info.cbs &= !HAS_ENCODE & !HAS_GET_FRAME_SIZE;
    reg_encoder(&mut r, &info, encoder_off::SIZE);
    r.expect_logged(&[(E, &enc_required("encode", "mtr_enc_no_both"))]);
    r.expect_no(Kind::Encoder, "mtr_enc_no_both", types, true);
}

#[test]
fn encoder_texture_scaling_order() {
    let mut r = Registry::default();
    let mut types = r.count(Kind::Encoder);

    let mut info = full_encoder(
        "mtr_enc_texscale",
        ENCODER_VIDEO,
        CAP_PASS_TEXTURE | CAP_SCALING,
    );
    info.cbs &= !HAS_GET_NAME;
    reg_encoder(&mut r, &info, encoder_off::SIZE + 8);
    r.expect_logged(&[(W, &texture_scaling("mtr_enc_texscale"))]);
    r.expect_no(Kind::Encoder, "mtr_enc_texscale", types, true);

    let mut info = full_encoder("mtr_enc_dupscale", ENCODER_VIDEO, 0);
    reg_encoder(&mut r, &info, encoder_off::SIZE);
    r.expect_logged(&[]);
    types += 1;
    info.flags = CAP_PASS_TEXTURE | CAP_SCALING;
    reg_encoder(&mut r, &info, encoder_off::SIZE);
    r.expect_logged(&[(
        W,
        &dup("obs_register_encoder", "Encoder", "mtr_enc_dupscale"),
    )]);
    assert_eq!(r.count(Kind::Encoder), types);
    r.expect_freed(true);
    r.expect_encoder("mtr_enc_dupscale", ENCODER_VIDEO, 0);
}

#[test]
fn encoder_size() {
    let mut r = Registry::default();
    let mut types = r.count(Kind::Encoder);
    let full = encoder_off::SIZE;
    let caps_off = encoder_off::CAPS;

    reg_encoder(&mut r, &full_encoder("mtr_enc_size0", ENCODER_VIDEO, 0), 0);
    r.expect_logged(&[(E, &enc_required("get_name", "mtr_enc_size0"))]);
    r.expect_no(Kind::Encoder, "mtr_enc_size0", types, false);

    reg_encoder(
        &mut r,
        &full_encoder("mtr_enc_size_at", ENCODER_AUDIO, 0),
        full,
    );
    r.expect_logged(&[]);
    r.expect_encoder("mtr_enc_size_at", ENCODER_AUDIO, 0);
    types += 1;

    let info = full_encoder("mtr_enc_size_above", ENCODER_AUDIO, 0);
    reg_encoder(&mut r, &info, full + 1);
    r.expect_logged(&[(E, &too_big("obs_encoder_info", full + 1, full))]);
    r.expect_no(Kind::Encoder, "mtr_enc_size_above", types, true);

    let frame_size_end = encoder_off::GET_FRAME_SIZE + PTR;
    let info = full_encoder("mtr_enc_size_fs", ENCODER_AUDIO, 0);
    reg_encoder(&mut r, &info, frame_size_end - 1);
    r.expect_logged(&[(E, &enc_required("get_frame_size", "mtr_enc_size_fs"))]);
    r.expect_no(Kind::Encoder, "mtr_enc_size_fs", types, false);

    let info = full_encoder("mtr_enc_size_fs_video", ENCODER_VIDEO, 0);
    reg_encoder(&mut r, &info, frame_size_end - 1);
    r.expect_logged(&[]);
    let entry = r.find(Kind::Encoder, "mtr_enc_size_fs_video").unwrap();
    assert_eq!(entry.encoder_type, ENCODER_VIDEO);
    // Its type data was past the size.
    assert!(!entry.type_data);
    types += 1;

    // The caps are the caller's whatever the size (characterized, not
    // endorsed).
    let info = full_encoder(
        "mtr_enc_caps_cut",
        ENCODER_VIDEO,
        CAP_PASS_TEXTURE | CAP_SCALING,
    );
    reg_encoder(&mut r, &info, caps_off);
    r.expect_logged(&[(W, &texture_scaling("mtr_enc_caps_cut"))]);
    r.expect_no(Kind::Encoder, "mtr_enc_caps_cut", types, true);

    let info = full_encoder("mtr_enc_caps_cut2", ENCODER_VIDEO, CAP_PASS_TEXTURE);
    reg_encoder(&mut r, &info, caps_off);
    r.expect_logged(&[(E, &neither("mtr_enc_caps_cut2"))]);
    r.expect_no(Kind::Encoder, "mtr_enc_caps_cut2", types, true);

    let info = full_encoder(
        "mtr_enc_caps_cut3",
        ENCODER_VIDEO,
        CAP_SCALING | CAP_DYN_BITRATE,
    );
    reg_encoder(&mut r, &info, caps_off);
    r.expect_logged(&[]);
    r.expect_encoder("mtr_enc_caps_cut3", ENCODER_VIDEO, 0);
    types += 1;

    let tex_end = encoder_off::ENCODE_TEXTURE + PTR;
    let mut info = full_encoder("mtr_enc_tex_cut", ENCODER_VIDEO, CAP_PASS_TEXTURE);
    info.cbs &= !HAS_ENCODE;
    reg_encoder(&mut r, &info, tex_end);
    r.expect_logged(&[]);
    r.expect_encoder("mtr_enc_tex_cut", ENCODER_VIDEO, CAP_PASS_TEXTURE);
    types += 1;
    let mut info = full_encoder("mtr_enc_tex_cut2", ENCODER_VIDEO, CAP_PASS_TEXTURE);
    info.cbs &= !HAS_ENCODE_TEXTURE;
    reg_encoder(&mut r, &info, tex_end);
    r.expect_logged(&[(E, &neither("mtr_enc_tex_cut2"))]);
    r.expect_no(Kind::Encoder, "mtr_enc_tex_cut2", types, true);
}

#[test]
fn service() {
    let mut r = Registry::default();
    let mut types = r.count(Kind::Service);
    let full = service_off::SIZE;

    reg_service(&mut r, &full_service("mtr_svc"), full);
    r.expect_logged(&[]);
    r.expect_freed(false);
    r.expect_service("mtr_svc");
    types += 1;

    let cases: [(u32, &str, &str); 4] = [
        (HAS_GET_NAME, "mtr_svc_no_get_name", "get_name"),
        (HAS_CREATE, "mtr_svc_no_create", "create"),
        (HAS_DESTROY, "mtr_svc_no_destroy", "destroy"),
        (HAS_GET_PROTOCOL, "mtr_svc_no_get_protocol", "get_protocol"),
    ];
    for (bit, id, val) in cases {
        let mut info = full_service(id);
        info.cbs &= !bit;
        reg_service(&mut r, &info, full);
        r.expect_logged(&[(E, &svc_required(val, id))]);
        r.expect_no(Kind::Service, id, types, true);
    }

    let mut info = full_service("mtr_svc");
    info.cbs &= !HAS_GET_NAME;
    reg_service(&mut r, &info, full + 8);
    r.expect_logged(&[(W, &dup("obs_register_service", "Service", "mtr_svc"))]);
    assert_eq!(r.count(Kind::Service), types);
    r.expect_freed(true);
    r.expect_service("mtr_svc");

    reg_service(&mut r, &full_service("mtr_svc_size0"), 0);
    r.expect_logged(&[(E, &svc_required("get_name", "mtr_svc_size0"))]);
    r.expect_no(Kind::Service, "mtr_svc_size0", types, false);

    reg_service(&mut r, &full_service("mtr_svc_size_above"), full + 1);
    r.expect_logged(&[(E, &too_big("obs_service_info", full + 1, full))]);
    r.expect_no(Kind::Service, "mtr_svc_size_above", types, true);

    let protocol_end = service_off::GET_PROTOCOL + PTR;
    let info = full_service("mtr_svc_size_cut");
    reg_service(&mut r, &info, protocol_end - 1);
    r.expect_logged(&[(E, &svc_required("get_protocol", "mtr_svc_size_cut"))]);
    r.expect_no(Kind::Service, "mtr_svc_size_cut", types, true);

    reg_service(&mut r, &info, protocol_end);
    r.expect_logged(&[]);
    r.expect_service("mtr_svc_size_cut");
}

// ================================================================= edges

#[test]
fn callbacks_from_bits() {
    assert_eq!(Callbacks::from_bits(0), Callbacks::default());
    let all = Callbacks::from_bits(u32::MAX);
    assert_eq!(all, Callbacks::from_bits(ALL_CBS));
    for (i, (bit, _)) in CBS.iter().enumerate() {
        assert_eq!(*bit, 1 << i);
        let one = Callbacks::from_bits(*bit);
        let set = [
            one.get_name,
            one.create,
            one.destroy,
            one.start,
            one.stop,
            one.protocols,
            one.encoded_packet,
            one.raw_video,
            one.raw_audio,
            one.raw_audio2,
            one.encode,
            one.encode_texture,
            one.encode_texture2,
            one.get_frame_size,
            one.get_protocol,
        ];
        for (j, s) in set.iter().enumerate() {
            assert_eq!(*s, i == j, "bit {i} field {j}");
        }
    }
    assert_eq!(Callbacks::from_bits(0xFFFF_8000), Callbacks::default());
}

#[test]
fn constants() {
    assert_eq!((OUT_V, OUT_A, OUT_E, OUT_S, OUT_M), (1, 2, 4, 8, 16));
    assert_eq!((ENCODER_AUDIO, ENCODER_VIDEO), (0, 1));
    assert_eq!((CAP_PASS_TEXTURE, CAP_SCALING), (2, 32));
}

#[test]
fn size_limit_is_slice_four_helper() {
    for full in [output_off::SIZE, encoder_off::SIZE, service_off::SIZE] {
        assert!(size_supported(0, full));
        assert!(size_supported(full - 1, full));
        assert!(size_supported(full, full));
        assert!(!size_supported(full + 1, full));
        assert!(!size_supported(usize::MAX, full));
    }
}

// ============================================================= proptests

fn any_flags() -> impl Strategy<Value = u32> {
    prop_oneof![0u32..0x80, any::<u32>()]
}

fn any_cbs() -> impl Strategy<Value = u32> {
    prop_oneof![
        Just(ALL_CBS),
        any::<u32>(),
        (0..15u32).prop_map(|i| ALL_CBS & !(1 << i)),
    ]
}

/// The rule broken names a callback the type lacks.
fn lacks(bits: u32, broken: Broken) -> bool {
    match broken {
        Broken::TextureScaling => true,
        Broken::NoEncodeTexture => bits & (HAS_ENCODE_TEXTURE | HAS_ENCODE_TEXTURE2) == 0,
        other => CBS.iter().any(|(b, br)| *br == other && bits & b == 0),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn output_reads_only_its_flags(flags in any_flags(), bits in any_cbs()) {
        prop_assert_eq!(out(flags, bits), out(flags & 0x1f, bits));
        prop_assert_eq!(out(flags, bits), out(flags, bits & OUTPUT_CBS));
    }

    #[test]
    fn output_more_callbacks_never_hurt(flags in any_flags(), bits in any_cbs(), more in any::<u32>()) {
        if out(flags, bits).is_ok() {
            prop_assert_eq!(out(flags, bits | more), Ok(()));
        }
    }

    #[test]
    fn output_reports_a_lacking_callback(flags in any_flags(), bits in any_cbs()) {
        if let Err(broken) = out(flags, bits) {
            prop_assert!(broken != Broken::TextureScaling);
            prop_assert!(lacks(bits, broken));
            // Adding it moves on.
            let bit = CBS.iter().find(|(_, br)| *br == broken).map(|(b, _)| *b).unwrap();
            prop_assert_ne!(out(flags, bits | bit), Err(broken));
        }
    }

    #[test]
    fn encoder_texture_scaling_first(t in any::<u32>(), caps in any_flags(), bits in any_cbs()) {
        let both = caps & CAP_PASS_TEXTURE != 0 && caps & CAP_SCALING != 0;
        prop_assert_eq!(enc(t, caps, bits) == Err(Broken::TextureScaling), both);
    }

    #[test]
    fn encoder_any_other_type_is_video(t in 1u32.., caps in any_flags(), bits in any_cbs()) {
        prop_assert_eq!(enc(t, caps, bits), enc(ENCODER_VIDEO, caps, bits));
        prop_assert_eq!(enc(t, caps, bits), enc(t, caps, bits & ENCODER_CBS));
    }

    #[test]
    fn encoder_reports_a_lacking_callback(t in any::<u32>(), caps in any_flags(), bits in any_cbs()) {
        if let Err(broken) = enc(t, caps, bits) {
            prop_assert!(lacks(bits, broken));
        }
        if enc(t, caps, bits).is_ok() {
            prop_assert_eq!(enc(t, caps, bits | ALL_CBS), Ok(()));
        }
    }

    #[test]
    fn service_needs_four(bits in any_cbs()) {
        let ok = bits & SERVICE_CBS == SERVICE_CBS;
        prop_assert_eq!(svc(bits).is_ok(), ok);
        if let Err(broken) = svc(bits) {
            prop_assert!(lacks(bits, broken));
        }
    }

    #[test]
    fn protocol_listed_is_byte_equality(
        protocol in proptest::collection::vec(1u8.., 0..6),
        list in proptest::collection::vec(proptest::collection::vec(1u8.., 0..6), 0..6),
    ) {
        let want = list.contains(&protocol);
        prop_assert_eq!(protocol_listed(&protocol, list.iter().map(Vec::as_slice)), want);
    }
}
