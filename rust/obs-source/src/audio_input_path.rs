//! Safe core of `libobs/obs-source-audio-input-path.c.h`: the path that takes one
//! audio packet from a source, `obs_source_output_audio()`, moved out of
//! `libobs/obs-source.c.h`.
//!
//! The decisions are [`audio_input`](crate::audio_input), on plain values. This
//! module makes the C's calls around them in its order and under its locks,
//! generic over a [`Source`]: one method per field read or write, lock and call
//! into the rest of libobs. The C shims live in the `obs-source-seam` crate, which
//! implements [`Source`] over `libobs/obs-source-audio-input-path-seam.h`, so this
//! crate calls nothing in libobs. The fields stay in `struct obs_source`, and each
//! read and write is C, so ThreadSanitizer keeps seeing them. The samples stay in
//! C: the resampler, the storage, the copy, the balance, the downmix and the
//! filters are one seam call each, and a plane is opaque here.
//!
//! What the C does and this keeps (characterized, not endorsed;
//! `test_source_audio_input` pins most of it):
//!
//! - When the resampler cannot be created, [`output_audio`] still sends the
//!   previous packet's samples on, with the previous packet's stamp.
//! - Only the resampler and the copy see the NULL of the unused planes. The
//!   filters get `audio_data`, which has the output's planes.
//! - `sample_info` is written here with no lock, and other threads read it.
//! - `resample_offset` is read and written under `audio_buf_mutex`, and the
//!   resampler runs on a copy with the mutex released.
//! - `next_resample_in_ts` is written only for a packet with a sample rate.
//!
//! One difference from the C, with no effect: the seam copies the filter's packet
//! to a `struct audio_data` after it takes `audio_mutex`, and the C does so just
//! before. `filter_mutex` is held for both, and no other thread has that packet.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable,
    clippy::todo
)]

use crate::audio_input::Format;

/// `MAX_AV_PLANES`, and `MAX_AUDIO_CHANNELS`, which the C clears up to.
pub const MAX_PLANES: usize = 8;

/// The plane pointers of a packet, opaque here. `None` is NULL.
pub type Planes<P> = [Option<P>; MAX_PLANES];

/// `struct obs_source_audio`, field for field: the shims read the caller's struct
/// as this. The C seam checks the layout.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Audio<P> {
    /// `data`.
    pub data: Planes<P>,
    /// `frames`.
    pub frames: u32,
    /// `speakers`, an `enum speaker_layout`.
    pub speakers: i32,
    /// `format`, an `enum audio_format`.
    pub format: i32,
    /// `samples_per_sec`.
    pub samples_per_sec: u32,
    /// `timestamp`.
    pub timestamp: u64,
}

/// What `audio_resampler_resample()` leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resampled<P> {
    /// `output`: NULL where the resampler wrote nothing.
    pub planes: Planes<P>,
    /// `frames`: the count passed in where the resampler wrote nothing.
    pub frames: u32,
    /// `resample_offset`, which the C stores back.
    pub offset: u64,
}

/// The source audio input seam: one method per field read or write, lock and
/// libobs call. A lock method is one lock call, and none of the others takes a
/// lock of its own, except [`Source::balance_value`], which is the public getter.
///
/// `resample_offset` needs `audio_buf_mutex`. [`Source::filter_audio`] needs
/// `filter_mutex`, and [`Source::output_audio_data`] needs `filter_mutex` and
/// `audio_mutex`.
pub trait Source {
    /// A plane pointer, opaque to the core.
    type Plane: Copy;
    /// The `const struct obs_source_audio *` of the caller, opaque to the core.
    type Input: Copy;
    /// The `struct obs_audio_data *` the filters returned, opaque to the core.
    type Output: Copy;

    /// `obs_source_valid(source, "obs_source_output_audio")`, which logs a NULL.
    fn valid(&self) -> bool;
    /// `destroying(source)`.
    fn destroying(&self) -> bool;
    /// `obs_ptr_valid(audio_in, "obs_source_output_audio")`, which logs a NULL.
    fn input_valid(&self, input: Self::Input) -> bool;
    /// `*audio_in`. The caller checked it with [`Source::input_valid`].
    fn read_input(&self, input: Self::Input) -> Audio<Self::Plane>;
    /// `get_audio_planes(format, speakers)`.
    fn audio_planes(&self, format: i32, speakers: i32) -> usize;

    /// The format, rate and speakers of `audio_output_get_info(obs->audio.audio)`.
    fn output_format(&self) -> Format;
    /// `audio_output_get_planes(obs->audio.audio)`.
    fn planes(&self) -> usize;
    /// `audio_output_get_block_size(obs->audio.audio)`.
    fn block_size(&self) -> usize;
    /// `audio_output_get_channels(obs->audio.audio)`.
    fn channels(&self) -> usize;

    /// `pthread_mutex_lock(&source->audio_buf_mutex)`.
    fn lock_audio_buf(&self);
    /// `pthread_mutex_unlock(&source->audio_buf_mutex)`.
    fn unlock_audio_buf(&self);
    /// `pthread_mutex_lock(&source->filter_mutex)`.
    fn lock_filter(&self);
    /// `pthread_mutex_unlock(&source->filter_mutex)`.
    fn unlock_filter(&self);
    /// `pthread_mutex_lock(&source->audio_mutex)`.
    fn lock_audio(&self);
    /// `pthread_mutex_unlock(&source->audio_mutex)`.
    fn unlock_audio(&self);

    /// `source->resampler != NULL`.
    fn has_resampler(&self) -> bool;
    /// `source->next_resample_in_ts`.
    fn next_resample_in_ts(&self) -> u64;
    /// `source->next_resample_in_ts = value`.
    fn set_next_resample_in_ts(&self, value: u64);
    /// The format, rate and speakers of `source->sample_info`.
    fn sample_info(&self) -> Format;
    /// `source->sample_info.speakers`.
    fn sample_speakers(&self) -> i32;
    /// The three stores to `source->sample_info`, in the C's order.
    fn set_sample_info(&self, value: Format);
    /// `source->last_async_compensation`.
    fn last_async_compensation(&self) -> bool;
    /// `source->last_async_compensation = value`.
    fn set_last_async_compensation(&self, value: bool);
    /// `os_atomic_load_bool(&source->async_compensation)`.
    fn async_compensation(&self) -> bool;
    /// `source->audio_failed`.
    fn audio_failed(&self) -> bool;
    /// `source->audio_failed = value`.
    fn set_audio_failed(&self, value: bool);
    /// `source->resample_offset`.
    fn resample_offset(&self) -> u64;
    /// `source->resample_offset = value`.
    fn set_resample_offset(&self, value: u64);
    /// `source->audio_storage_size`.
    fn audio_storage_size(&self) -> usize;
    /// `source->audio_storage_size = value`.
    fn set_audio_storage_size(&self, value: usize);
    /// `source->audio_data.frames = value`.
    fn set_data_frames(&self, value: u32);
    /// `source->audio_data.timestamp = value`.
    fn set_data_timestamp(&self, value: u64);
    /// `source->flags`.
    fn flags(&self) -> u32;

    /// `audio_resampler_destroy(source->resampler); source->resampler = NULL`.
    fn destroy_resampler(&self);
    /// `source->resampler = audio_resampler_create(&output_info,
    /// &source->sample_info)`: whether it is not NULL.
    fn create_resampler(&self, output: Format) -> bool;
    /// `blog(LOG_ERROR, "creation of resampler failed")`.
    fn log_resampler_failed(&self);
    /// `audio_resampler_resample(source->resampler, output, &frames, &offset,
    /// data, frames)`, from a NULL `output`.
    fn resample(
        &self,
        data: &Planes<Self::Plane>,
        frames: u32,
        offset: u64,
    ) -> Resampled<Self::Plane>;

    /// `bfree(source->audio_data.data[plane])`.
    fn free_storage(&self, plane: usize);
    /// `source->audio_data.data[plane] = bmalloc(size)`.
    fn alloc_storage(&self, plane: usize, size: usize);
    /// `memcpy(source->audio_data.data[plane], data, size)`.
    fn copy_to_storage(&self, plane: usize, data: Option<Self::Plane>, size: usize);

    /// `obs_source_get_balance_value(source)`, which takes `audio_buf_mutex`.
    fn balance_value(&self) -> f32;
    /// `obs_source_audio_input_balance(source->audio_data.data, frames, balance,
    /// OBS_BALANCE_TYPE_SINE_LAW)`.
    fn apply_balance(&self, frames: u32, balance: f32);
    /// `obs_source_audio_input_downmix(source->audio_data.data, channels, frames)`.
    fn downmix(&self, channels: usize, frames: u32);

    /// `filter_async_audio(source, &source->audio_data)`: `None` for NULL.
    fn filter_audio(&self) -> Option<Self::Output>;
    /// `source_output_audio_data(source, &data)`, with `data` copied from `output`.
    fn output_audio_data(&self, output: Self::Output);
}

/// `obs_source_output_audio()`. Not ported yet.
pub fn output_audio<S: Source>(_source: &S, _input: S::Input) {}
