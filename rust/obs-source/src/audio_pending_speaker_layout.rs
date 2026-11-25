//! `obs_source_audio_pending()` and `obs_source_get_speaker_layout()`: the
//! two audio getters scenes, transitions and audio filters read every tick
//! to learn whether a source's audio is still pending and which speaker
//! layout it produces.
use core::ffi::{CStr, c_int};

/// The public function named in the pending query's NULL diagnostic.
pub const PENDING_FUNCTION: &CStr = c"obs_source_audio_pending";
/// The name the speaker layout's NULL diagnostic carries: upstream hands
/// `"obs_source_get_audio_channels"` to its validity check (kept).
pub const SPEAKER_LAYOUT_FUNCTION: &CStr = c"obs_source_get_audio_channels";

/// `OBS_SOURCE_AUDIO` (`obs-source.h`).
pub const OBS_SOURCE_AUDIO: u32 = 1 << 1;
/// `OBS_SOURCE_COMPOSITE` (`obs-source.h`).
pub const OBS_SOURCE_COMPOSITE: u32 = 1 << 6;
/// `SPEAKERS_UNKNOWN` (`media-io/audio-io.h`).
pub const SPEAKERS_UNKNOWN: c_int = 0;

/// What the pending query reads of a live source, in the order it asks.
pub trait Pending {
    /// `obs_source_removed()` of the source.
    fn removed(&self) -> bool;
    /// `info.output_flags`.
    fn output_flags(&self) -> u32;
    /// `audio_pending`.
    fn audio_pending(&self) -> bool;
}

/// `None` is a NULL source: it logs once with [`PENDING_FUNCTION`] and
/// answers true without reading anything. A removed source answers true
/// without reading its flags; a source whose flags have neither
/// [`OBS_SOURCE_AUDIO`] nor [`OBS_SOURCE_COMPOSITE`] answers true without
/// reading `audio_pending`; otherwise the answer is `audio_pending`.
pub fn audio_pending(source: Option<&impl Pending>, log: impl FnOnce(&'static CStr)) -> bool {
    let Some(source) = source else {
        log(PENDING_FUNCTION);
        return true;
    };
    if source.removed() {
        return true;
    }
    if source.output_flags() & (OBS_SOURCE_COMPOSITE | OBS_SOURCE_AUDIO) == 0 {
        return true;
    }
    source.audio_pending()
}

/// `None` is a NULL source: it logs once with [`SPEAKER_LAYOUT_FUNCTION`]
/// and answers [`SPEAKERS_UNKNOWN`] without reading anything. Otherwise
/// `speakers` (`sample_info.speakers`) is read once and is the answer.
pub fn speaker_layout(
    speakers: Option<impl FnOnce() -> c_int>,
    log: impl FnOnce(&'static CStr),
) -> c_int {
    let Some(speakers) = speakers else {
        log(SPEAKER_LAYOUT_FUNCTION);
        return SPEAKERS_UNKNOWN;
    };
    speakers()
}
