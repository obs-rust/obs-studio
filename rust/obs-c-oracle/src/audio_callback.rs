//! `oracle_*` build of `libobs/obs-audio.c.h` (oracle/audio_callback.c):
//! the audio tick, `oracle_audio_callback()`, on the C oracles of its
//! decisions, with the core-audio seam (`libobs/obs-audio-seam.c.h`)
//! compiled under its own names, which the Rust shim calls too.
//!
//! A world is a stand-in core with its views, mixes, sources and task
//! queue. [`oracle_tick_use`] makes one the core the seam and the
//! stand-ins see; the stand-ins (`oracle_tick_*`) record every libobs call
//! in the world's log. One world runs at a time: the caller serializes.
//! Worlds, sources and mixes cross as opaque pointers and indexes.

use core::ffi::{c_char, c_int, c_void};

/// `struct oracle_tick_source_init`: the starting state of a test source.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct SourceInit {
    /// `info.type`.
    pub kind: u32,
    /// `info.output_flags`.
    pub output_flags: u32,
    /// `info.audio_render` is set.
    pub audio_render: bool,
    /// `obs_source_removed()` answers true.
    pub removed: bool,
    /// `obs_source_active()` answers true.
    pub active: bool,
    /// `obs_source_get_ref()` gives NULL.
    pub dead: bool,
    /// `audio_pending`.
    pub pending: bool,
    /// `pending_stop`.
    pub pending_stop: bool,
    /// `timing_set`.
    pub timing_set: bool,
    /// `audio_is_duplicated`.
    pub duplicated: bool,
    /// `muted`.
    pub muted: bool,
    /// `monitoring_enabled`.
    pub monitoring: bool,
    /// `audio_ts`.
    pub audio_ts: u64,
    /// Frames already in each channel's input.
    pub frames: u32,
    /// The mixes whose `audio_output_buf` exists.
    pub out_mixes: u32,
    /// `audio_mixers`.
    pub audio_mixers: u32,
    /// `volume`.
    pub volume: f32,
}

/// `struct oracle_tick_core_init`: the core's buffering fields.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CoreInit {
    /// `buffered_ts`.
    pub buffered_ts: u64,
    /// `buffering_wait_ticks`.
    pub buffering_wait_ticks: u64,
    /// `total_buffering_ticks`.
    pub total_buffering_ticks: c_int,
    /// `max_buffering_ticks`.
    pub max_buffering_ticks: c_int,
    /// `fixed_buffer`.
    pub fixed_buffer: bool,
}

/// `obs_source_enum_proc_t`.
pub type EnumProc =
    unsafe extern "C" fn(parent: *mut c_void, child: *mut c_void, param: *mut c_void);

unsafe extern "C" {
    /// A world at `rate` with `channels` audio channels.
    pub fn oracle_tick_new(rate: u32, channels: usize, init: *const CoreInit) -> *mut c_void;
    /// Frees a world and its sources.
    pub fn oracle_tick_free(world: *mut c_void);
    /// Adds a source; its index.
    pub fn oracle_tick_add_source(world: *mut c_void, init: *const SourceInit) -> c_int;
    /// What `obs_source_enum_active_tree()` visits for `source`, in order.
    pub fn oracle_tick_set_tree(world: *mut c_void, source: c_int, tree: *const c_int, len: usize);
    /// Links the audio source list in `order`.
    pub fn oracle_tick_link_audio(world: *mut c_void, order: *const c_int, len: usize);
    /// Adds a view; its index.
    pub fn oracle_tick_add_view(world: *mut c_void) -> c_int;
    /// Sets a view's channel to a source, or NULL for -1.
    pub fn oracle_tick_set_channel(world: *mut c_void, view: c_int, channel: u32, source: c_int);
    /// Adds a mix on a view (NULL for -1).
    pub fn oracle_tick_add_mix(world: *mut c_void, view: c_int, mix_audio: bool);
    /// Sets `monitoring_duplicating_source` (NULL for -1).
    pub fn oracle_tick_set_dedup(world: *mut c_void, source: c_int);
    /// Queues an audio task that, when run, queues task `id + 100`
    /// `chain` times down.
    pub fn oracle_tick_queue_task(world: *mut c_void, id: c_int, chain: c_int);
    /// Pushes `frames` more frames into each channel of a source's input,
    /// and sets its `audio_ts` when it has none.
    pub fn oracle_tick_feed(world: *mut c_void, source: c_int, frames: u32, audio_ts: u64);
    /// Makes `world` the core the seam and the stand-ins see.
    pub fn oracle_tick_use(world: *mut c_void);
    /// Clears the world's mixes; the pointer to its `MAX_AUDIO_MIXES`
    /// `struct audio_output_data`.
    pub fn oracle_tick_clear_mixes(world: *mut c_void) -> *mut c_void;
    /// One tick of the original C on `world`, its mixes cleared first.
    pub fn oracle_tick_run_c(
        world: *mut c_void,
        start: u64,
        end: u64,
        mixers: u32,
        out_ts: *mut u64,
    ) -> bool;
    /// The world's log and state, written to `out`; its length.
    pub fn oracle_tick_state(world: *mut c_void, out: *mut c_char, cap: usize) -> usize;

    /// Stand-in `audio_output_get_sample_rate()`.
    pub fn oracle_tick_sample_rate(audio: *const c_void) -> u32;
    /// Stand-in `audio_output_get_channels()`.
    pub fn oracle_tick_channels(audio: *const c_void) -> usize;
    /// Stand-in `obs_source_get_ref()`.
    pub fn oracle_tick_get_ref(source: *mut c_void) -> *mut c_void;
    /// Stand-in `obs_source_release()`.
    pub fn oracle_tick_release(source: *mut c_void);
    /// Stand-in `obs_source_get_name()`.
    pub fn oracle_tick_get_name(source: *const c_void) -> *const c_char;
    /// Stand-in `obs_source_removed()`.
    pub fn oracle_tick_removed(source: *const c_void) -> bool;
    /// Stand-in `obs_source_active()`.
    pub fn oracle_tick_active(source: *const c_void) -> bool;
    /// Stand-in `obs_source_audio_render()`.
    pub fn oracle_tick_audio_render(
        source: *mut c_void,
        mixers: u32,
        channels: usize,
        sample_rate: usize,
        size: usize,
    );
    /// Stand-in `obs_source_enum_active_tree()`.
    pub fn oracle_tick_enum_active_tree(
        source: *mut c_void,
        callback: EnumProc,
        param: *mut c_void,
    );
    /// Stand-in `blog()`.
    pub fn oracle_tick_blog(level: c_int, format: *const c_char, ...);
}
