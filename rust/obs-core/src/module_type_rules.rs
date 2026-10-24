//! Safe core of `libobs/obs-module-type-rules.c.h`: the output, encoder and
//! service registration rules.
//!
//! The pure decisions of `obs_register_output_s()`,
//! `obs_register_encoder_s()` and `obs_register_service_s()`, in the order
//! they make them: the first callback an output type lacks for its flags
//! ([`output_check`]), a texture encoder that scales itself and the first
//! callback an encoder type lacks ([`encoder_check`]), the first callback a
//! service type lacks ([`service_check`]), and whether a protocol of a
//! service output is already registered ([`protocol_listed`]). The size
//! limit is slice 4's [`size_supported`](crate::module_source_rules::size_supported),
//! which `REGISTER_OBS_DEF()` calls for all three.
//!
//! `obs-module.c` keeps the duplicate lookups, the size-aware `HAS_VAL()`
//! checks (passed in as [`Callbacks`]), the copy and the type lists, the
//! protocol split and list, the loading module's lists, every log message and
//! the error path, and passes the values in.
//!
//! Flags, capabilities and types are plain `u32`s, as a plugin can pass any
//! value: bits the rules do not name are ignored, and an encoder type other
//! than audio is treated as video. Protocols are bytes, not text. Nothing
//! here panics, indexes or overflows for any input.
//!
//! Kept as the C has it, characterized and not endorsed (#1876):
//!
//! - `OBS_OUTPUT_MULTI_TRACK` without `OBS_OUTPUT_AUDIO` needs no audio
//!   callback, and an encoded output needs neither raw callback, whatever
//!   its video and audio flags.
//! - The C passes the caller's encoder type and capabilities, read from the
//!   caller's structure whatever its size: a texture encoder that scales
//!   itself is refused even when the size ends before its capabilities.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

/// `OBS_OUTPUT_VIDEO`.
pub const OUTPUT_VIDEO: u32 = 1 << 0;
/// `OBS_OUTPUT_AUDIO`.
pub const OUTPUT_AUDIO: u32 = 1 << 1;
/// `OBS_OUTPUT_ENCODED`.
pub const OUTPUT_ENCODED: u32 = 1 << 2;
/// `OBS_OUTPUT_SERVICE`.
pub const OUTPUT_SERVICE: u32 = 1 << 3;
/// `OBS_OUTPUT_MULTI_TRACK`.
pub const OUTPUT_MULTI_TRACK: u32 = 1 << 4;

/// `OBS_ENCODER_AUDIO`.
pub const ENCODER_AUDIO: u32 = 0;
/// `OBS_ENCODER_VIDEO`.
pub const ENCODER_VIDEO: u32 = 1;

/// `OBS_ENCODER_CAP_PASS_TEXTURE`.
pub const CAP_PASS_TEXTURE: u32 = 1 << 1;
/// `OBS_ENCODER_CAP_SCALING`.
pub const CAP_SCALING: u32 = 1 << 5;

/// `OBS_MODULE_TYPE_RULES_HAS_GET_NAME`.
pub const HAS_GET_NAME: u32 = 1 << 0;
/// `OBS_MODULE_TYPE_RULES_HAS_CREATE`.
pub const HAS_CREATE: u32 = 1 << 1;
/// `OBS_MODULE_TYPE_RULES_HAS_DESTROY`.
pub const HAS_DESTROY: u32 = 1 << 2;
/// `OBS_MODULE_TYPE_RULES_HAS_START`.
pub const HAS_START: u32 = 1 << 3;
/// `OBS_MODULE_TYPE_RULES_HAS_STOP`.
pub const HAS_STOP: u32 = 1 << 4;
/// `OBS_MODULE_TYPE_RULES_HAS_PROTOCOLS`.
pub const HAS_PROTOCOLS: u32 = 1 << 5;
/// `OBS_MODULE_TYPE_RULES_HAS_ENCODED_PACKET`.
pub const HAS_ENCODED_PACKET: u32 = 1 << 6;
/// `OBS_MODULE_TYPE_RULES_HAS_RAW_VIDEO`.
pub const HAS_RAW_VIDEO: u32 = 1 << 7;
/// `OBS_MODULE_TYPE_RULES_HAS_RAW_AUDIO`.
pub const HAS_RAW_AUDIO: u32 = 1 << 8;
/// `OBS_MODULE_TYPE_RULES_HAS_RAW_AUDIO2`.
pub const HAS_RAW_AUDIO2: u32 = 1 << 9;
/// `OBS_MODULE_TYPE_RULES_HAS_ENCODE`.
pub const HAS_ENCODE: u32 = 1 << 10;
/// `OBS_MODULE_TYPE_RULES_HAS_ENCODE_TEXTURE`.
pub const HAS_ENCODE_TEXTURE: u32 = 1 << 11;
/// `OBS_MODULE_TYPE_RULES_HAS_ENCODE_TEXTURE2`.
pub const HAS_ENCODE_TEXTURE2: u32 = 1 << 12;
/// `OBS_MODULE_TYPE_RULES_HAS_GET_FRAME_SIZE`.
pub const HAS_GET_FRAME_SIZE: u32 = 1 << 13;
/// `OBS_MODULE_TYPE_RULES_HAS_GET_PROTOCOL`.
pub const HAS_GET_PROTOCOL: u32 = 1 << 14;

/// The callbacks `HAS_VAL()` found: set, and whole inside the caller's
/// size. Each rule reads only those of its type.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Callbacks {
    pub get_name: bool,
    pub create: bool,
    pub destroy: bool,
    /// Outputs.
    pub start: bool,
    pub stop: bool,
    pub protocols: bool,
    pub encoded_packet: bool,
    pub raw_video: bool,
    pub raw_audio: bool,
    pub raw_audio2: bool,
    /// Encoders.
    pub encode: bool,
    pub encode_texture: bool,
    pub encode_texture2: bool,
    pub get_frame_size: bool,
    /// Services.
    pub get_protocol: bool,
}

impl Callbacks {
    /// From the C's `OBS_MODULE_TYPE_RULES_HAS_*` bits; other bits are
    /// ignored.
    pub const fn from_bits(bits: u32) -> Self {
        Self {
            get_name: bits & HAS_GET_NAME != 0,
            create: bits & HAS_CREATE != 0,
            destroy: bits & HAS_DESTROY != 0,
            start: bits & HAS_START != 0,
            stop: bits & HAS_STOP != 0,
            protocols: bits & HAS_PROTOCOLS != 0,
            encoded_packet: bits & HAS_ENCODED_PACKET != 0,
            raw_video: bits & HAS_RAW_VIDEO != 0,
            raw_audio: bits & HAS_RAW_AUDIO != 0,
            raw_audio2: bits & HAS_RAW_AUDIO2 != 0,
            encode: bits & HAS_ENCODE != 0,
            encode_texture: bits & HAS_ENCODE_TEXTURE != 0,
            encode_texture2: bits & HAS_ENCODE_TEXTURE2 != 0,
            get_frame_size: bits & HAS_GET_FRAME_SIZE != 0,
            get_protocol: bits & HAS_GET_PROTOCOL != 0,
        }
    }
}

/// The first rule a type breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Broken {
    /// A texture encoder (`OBS_ENCODER_CAP_PASS_TEXTURE`) that scales itself
    /// (`OBS_ENCODER_CAP_SCALING`).
    TextureScaling,
    /// No `get_name`.
    NoGetName,
    /// No `create`.
    NoCreate,
    /// No `destroy`.
    NoDestroy,
    /// An output without `start`.
    NoStart,
    /// An output without `stop`.
    NoStop,
    /// A service output without `protocols`.
    NoProtocols,
    /// An encoded output without `encoded_packet`.
    NoEncodedPacket,
    /// A raw video output without `raw_video`.
    NoRawVideo,
    /// A raw single-track audio output without `raw_audio`.
    NoRawAudio,
    /// A raw multi-track audio output without `raw_audio2`.
    NoRawAudio2,
    /// A texture encoder with neither `encode_texture` nor
    /// `encode_texture2`.
    NoEncodeTexture,
    /// Any other encoder without `encode`.
    NoEncode,
    /// An audio encoder without `get_frame_size`.
    NoGetFrameSize,
    /// A service without `get_protocol`.
    NoGetProtocol,
}

/// `Err(broken)` unless `present`.
const fn need(present: bool, broken: Broken) -> Result<(), Broken> {
    if present { Ok(()) } else { Err(broken) }
}

/// get_name, create and destroy, which every type needs first.
const fn common(callbacks: Callbacks) -> Result<(), Broken> {
    if !callbacks.get_name {
        return Err(Broken::NoGetName);
    }
    if !callbacks.create {
        return Err(Broken::NoCreate);
    }
    need(callbacks.destroy, Broken::NoDestroy)
}

/// Same contract as C `obs_module_type_rules_output_check`: get_name,
/// create, destroy, start and stop; protocols for a service output;
/// encoded_packet for an encoded output, else raw_video for video and
/// raw_audio2 (multi-track) or raw_audio for audio.
pub fn output_check(flags: u32, callbacks: Callbacks) -> Result<(), Broken> {
    common(callbacks)?;
    need(callbacks.start, Broken::NoStart)?;
    need(callbacks.stop, Broken::NoStop)?;
    if flags & OUTPUT_SERVICE != 0 {
        need(callbacks.protocols, Broken::NoProtocols)?;
    }
    if flags & OUTPUT_ENCODED != 0 {
        return need(callbacks.encoded_packet, Broken::NoEncodedPacket);
    }
    if flags & OUTPUT_VIDEO != 0 {
        need(callbacks.raw_video, Broken::NoRawVideo)?;
    }
    if flags & OUTPUT_AUDIO != 0 {
        if flags & OUTPUT_MULTI_TRACK != 0 {
            need(callbacks.raw_audio2, Broken::NoRawAudio2)?;
        } else {
            need(callbacks.raw_audio, Broken::NoRawAudio)?;
        }
    }
    Ok(())
}

/// Same contract as C `obs_module_type_rules_encoder_check`: a texture
/// encoder that scales itself first, whatever its callbacks; then get_name,
/// create, destroy; encode_texture or encode_texture2 for a texture encoder,
/// else encode; get_frame_size for an audio encoder.
pub fn encoder_check(encoder_type: u32, caps: u32, callbacks: Callbacks) -> Result<(), Broken> {
    let texture = caps & CAP_PASS_TEXTURE != 0;
    if texture && caps & CAP_SCALING != 0 {
        return Err(Broken::TextureScaling);
    }
    common(callbacks)?;
    if texture {
        need(
            callbacks.encode_texture || callbacks.encode_texture2,
            Broken::NoEncodeTexture,
        )?;
    } else {
        need(callbacks.encode, Broken::NoEncode)?;
    }
    if encoder_type == ENCODER_AUDIO {
        need(callbacks.get_frame_size, Broken::NoGetFrameSize)?;
    }
    Ok(())
}

/// Same contract as C `obs_module_type_rules_service_check`: get_name,
/// create, destroy and get_protocol.
pub fn service_check(callbacks: Callbacks) -> Result<(), Broken> {
    common(callbacks)?;
    need(callbacks.get_protocol, Broken::NoGetProtocol)
}

/// Same contract as C `obs_module_type_rules_protocol_listed`: `protocol`
/// is one of `protocols`, byte for byte (`strcmp() == 0`).
pub fn protocol_listed<'a>(protocol: &[u8], protocols: impl IntoIterator<Item = &'a [u8]>) -> bool {
    protocols.into_iter().any(|listed| listed == protocol)
}
