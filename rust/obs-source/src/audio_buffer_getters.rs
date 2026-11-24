//! `obs_source_get_audio_timestamp()` and `obs_source_get_audio_mix()`: the
//! two getters scenes and transitions call every audio tick to read a
//! source's audio buffer timestamp and its output buffer pointers.
use core::ffi::CStr;

/// The public function named in the timestamp's NULL diagnostic.
pub const TIMESTAMP_FUNCTION: &CStr = c"obs_source_get_audio_timestamp";
/// The public function named in the mix's NULL source diagnostic.
pub const MIX_FUNCTION: &CStr = c"obs_source_get_audio_mix";
/// The parameter named in a NULL source diagnostic.
pub const SOURCE: &CStr = c"source";
/// `obs_ptr_valid(audio, "audio")`: the parameter name stands in both places
/// of a NULL `audio` diagnostic (upstream's quirk, kept).
pub const AUDIO: &CStr = c"audio";

/// `MAX_AUDIO_MIXES` (`media-io/audio-io.h`).
pub const MAX_AUDIO_MIXES: usize = 6;
/// `MAX_AUDIO_CHANNELS` (`media-io/audio-io.h`).
pub const MAX_AUDIO_CHANNELS: usize = 8;

/// `struct obs_source_audio_mix`: one buffer per mix and channel.
pub type Mix<B> = [[B; MAX_AUDIO_CHANNELS]; MAX_AUDIO_MIXES];

/// The source's audio timestamp and the mutex it is read under.
pub trait Timestamp {
    /// Take `audio_buf_mutex`.
    fn lock(&mut self);
    /// Read `audio_ts`; called only while the mutex is held.
    fn audio_ts(&self) -> u64;
    /// Release `audio_buf_mutex`.
    fn unlock(&mut self);
}

/// The source's output buffers, `audio_output_buf[mix][channel]`.
pub trait OutputBuffers {
    /// A buffer pointer, copied, never read through.
    type Buffer: Copy;
    /// Read `audio_output_buf[mix][channel]`.
    fn output_buf(&self, mix: usize, channel: usize) -> Self::Buffer;
}

/// `None` is a NULL source: it logs once with [`TIMESTAMP_FUNCTION`] and
/// [`SOURCE`] and answers 0 without touching anything. Otherwise `audio_ts`
/// is read once, between taking and releasing the mutex.
pub fn timestamp(
    source: Option<&mut impl Timestamp>,
    log: impl FnOnce(&'static CStr, &'static CStr),
) -> u64 {
    let Some(source) = source else {
        log(TIMESTAMP_FUNCTION, SOURCE);
        return 0;
    };
    source.lock();
    let timestamp = source.audio_ts();
    source.unlock();
    timestamp
}

/// A NULL source logs once with [`MIX_FUNCTION`] and [`SOURCE`] (whatever
/// `audio` is); a NULL `audio` logs once with [`AUDIO`] twice. Neither
/// writes anything. Otherwise every `audio[mix][channel]` becomes the
/// source's `audio_output_buf[mix][channel]`, each read once.
pub fn mix<S: OutputBuffers>(
    source: Option<&S>,
    audio: Option<&mut Mix<S::Buffer>>,
    log: impl FnOnce(&'static CStr, &'static CStr),
) {
    let Some(source) = source else {
        log(MIX_FUNCTION, SOURCE);
        return;
    };
    let Some(audio) = audio else {
        log(AUDIO, AUDIO);
        return;
    };
    for (mix, output) in audio.iter_mut().enumerate() {
        for (channel, data) in output.iter_mut().enumerate() {
            *data = source.output_buf(mix, channel);
        }
    }
}
