//! Safe core of `libobs/obs-module-source-rules.c.h`: the source
//! registration rules.
//!
//! The pure decisions of `obs_register_source_s()`, in the order it makes
//! them: the list a source type goes in ([`list_of`]), the size limit
//! ([`size_supported`]), the output flag rules ([`output_flags`]), the
//! composite and required callback checks ([`check`]) and the versioned id
//! ([`versioned_id`]). `obs-module.c` keeps the duplicate lookup, the
//! module's source list, the copy of the caller's structure, the size-aware
//! `HAS_VAL()` checks (passed in as [`Callbacks`]), the type lists, every log
//! message and the error path, and passes the values in.
//!
//! Types and flags are plain `u32`s, as a plugin can pass any value: an
//! unknown type is [`List::Unknown`], and flag bits the rules do not name
//! are kept. The id is bytes, not text, with `None` for NULL. Nothing here
//! panics, indexes or overflows for any input.
//!
//! Kept as the C has it, characterized and not endorsed (#1876):
//!
//! - The C reads the caller's structure for the list ([`list_of`]) and for
//!   the get_width/get_height rule of [`check`] (type and flags), and its
//!   copy, cut at the caller's size, for [`output_flags`] and the composite
//!   rules. Normalization only changes a filter's or a transition's flags,
//!   which the get_width/get_height rule skips, so the two reads differ only
//!   when the size cuts the copy before its type or flags end. Such a source
//!   always fails (get_name ends after them), so the two reads change which
//!   rule is reported, never whether a source is registered.
//! - [`versioned_id`] prints the version as an `int`: `2147483648` is
//!   `_v-2147483648` and `u32::MAX` is `_v-1`.

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

/// `OBS_SOURCE_TYPE_INPUT`.
pub const TYPE_INPUT: u32 = 0;
/// `OBS_SOURCE_TYPE_FILTER`.
pub const TYPE_FILTER: u32 = 1;
/// `OBS_SOURCE_TYPE_TRANSITION`.
pub const TYPE_TRANSITION: u32 = 2;
/// `OBS_SOURCE_TYPE_SCENE`.
pub const TYPE_SCENE: u32 = 3;

/// `OBS_SOURCE_VIDEO`.
pub const VIDEO: u32 = 1 << 0;
/// `OBS_SOURCE_AUDIO`.
pub const AUDIO: u32 = 1 << 1;
/// `OBS_SOURCE_ASYNC`.
pub const ASYNC: u32 = 1 << 2;
/// `OBS_SOURCE_CUSTOM_DRAW`.
pub const CUSTOM_DRAW: u32 = 1 << 3;
/// `OBS_SOURCE_COMPOSITE`.
pub const COMPOSITE: u32 = 1 << 6;

/// The list a source type goes in, besides `obs->source_types`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum List {
    /// Not a source type: rejected.
    Unknown,
    /// A scene: no list of its own.
    None,
    /// `obs->input_types`.
    Input,
    /// `obs->filter_types`.
    Filter,
    /// `obs->transition_types`.
    Transition,
}

/// Same contract as C `obs_module_source_rules_list_of`.
pub fn list_of(source_type: u32) -> List {
    match source_type {
        TYPE_INPUT => List::Input,
        TYPE_FILTER => List::Filter,
        TYPE_TRANSITION => List::Transition,
        TYPE_SCENE => List::None,
        _ => List::Unknown,
    }
}

/// Same contract as C `obs_module_source_rules_size_supported`: the
/// caller's size is not above the one libobs knows.
pub fn size_supported(size: usize, supported: usize) -> bool {
    size <= supported
}

/// Same contract as C `obs_module_source_rules_output_flags`: a filter
/// without video is made async, a transition composite, video and custom
/// draw; every other bit is kept.
pub fn output_flags(source_type: u32, flags: u32) -> u32 {
    match source_type {
        TYPE_FILTER if flags & VIDEO == 0 => flags | ASYNC,
        TYPE_TRANSITION => flags | COMPOSITE | VIDEO | CUSTOM_DRAW,
        _ => flags,
    }
}

/// `OBS_MODULE_SOURCE_RULES_HAS_GET_NAME`.
pub const HAS_GET_NAME: u32 = 1 << 0;
/// `OBS_MODULE_SOURCE_RULES_HAS_GET_WIDTH`.
pub const HAS_GET_WIDTH: u32 = 1 << 1;
/// `OBS_MODULE_SOURCE_RULES_HAS_GET_HEIGHT`.
pub const HAS_GET_HEIGHT: u32 = 1 << 2;
/// `OBS_MODULE_SOURCE_RULES_HAS_AUDIO_RENDER`.
pub const HAS_AUDIO_RENDER: u32 = 1 << 3;

/// The callbacks `HAS_VAL()` found: set, and whole inside the caller's
/// size.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Callbacks {
    pub get_name: bool,
    pub get_width: bool,
    pub get_height: bool,
    pub audio_render: bool,
}

impl Callbacks {
    /// From the C's `OBS_MODULE_SOURCE_RULES_HAS_*` bits; other bits are
    /// ignored.
    pub const fn from_bits(bits: u32) -> Self {
        Self {
            get_name: bits & HAS_GET_NAME != 0,
            get_width: bits & HAS_GET_WIDTH != 0,
            get_height: bits & HAS_GET_HEIGHT != 0,
            audio_render: bits & HAS_AUDIO_RENDER != 0,
        }
    }
}

/// The first rule a source breaks, in the C's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Broken {
    /// Composite and audio.
    CompositeAudio,
    /// Composite and async.
    CompositeAsync,
    /// No `get_name`.
    NoGetName,
    /// A sync video source that is not a filter or a transition, without
    /// `get_width`.
    NoGetWidth,
    /// The same, without `get_height`.
    NoGetHeight,
    /// Composite, without `audio_render`.
    NoAudioRender,
}

/// Same contract as C `obs_module_source_rules_check`. `source_type` and
/// `info_flags` are the caller's (the get_width/get_height rule), `flags`
/// the normalized ones of [`output_flags`] (the composite rules).
pub fn check(
    source_type: u32,
    info_flags: u32,
    flags: u32,
    callbacks: Callbacks,
) -> Result<(), Broken> {
    let composite = flags & COMPOSITE != 0;
    if composite && flags & AUDIO != 0 {
        return Err(Broken::CompositeAudio);
    }
    if composite && flags & ASYNC != 0 {
        return Err(Broken::CompositeAsync);
    }
    if !callbacks.get_name {
        return Err(Broken::NoGetName);
    }
    let sync_video = info_flags & VIDEO != 0 && info_flags & ASYNC == 0;
    if source_type != TYPE_FILTER && source_type != TYPE_TRANSITION && sync_video {
        if !callbacks.get_width {
            return Err(Broken::NoGetWidth);
        }
        if !callbacks.get_height {
            return Err(Broken::NoGetHeight);
        }
    }
    if composite && !callbacks.audio_render {
        return Err(Broken::NoAudioRender);
    }
    Ok(())
}

/// What the C's `"%s"` prints for a NULL id (glibc, the MSVC CRT and
/// macOS's libc all print it). libobs never gets there: a NULL id crashes
/// in the duplicate lookup first, and a size that cuts the id off lacks
/// get_name.
pub const NULL_ID: &[u8] = b"(null)";

/// Same contract as C `obs_module_source_rules_versioned_id`: for a
/// version, the id, `_v` and the version printed as an `int`; for version
/// 0, a copy of the id (`None` for NULL).
pub fn versioned_id(id: Option<&[u8]>, version: u32) -> Option<Vec<u8>> {
    if version == 0 {
        return id.map(<[u8]>::to_vec);
    }
    let mut out = id.unwrap_or(NULL_ID).to_vec();
    out.extend_from_slice(b"_v");
    out.extend_from_slice(version.cast_signed().to_string().as_bytes());
    Some(out)
}
