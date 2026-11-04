//! `oracle_*` builds of `libobs/obs-source-audio-input-path.c.h`
//! (oracle/source_audio_input_path.c): the source audio input path, renamed, with
//! the seam (`libobs/obs-source-audio-input-path-seam.c.h`) compiled under its own
//! names, which the Rust shim calls too. A test source is a zeroed
//! `struct obs_source` with its mutexes and a log of each call. Sources and
//! packets cross as opaque pointers.

use core::ffi::{c_char, c_int, c_void};

/// `struct oracle_audio_input_path_init`: the starting fields of a test source.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Init {
    /// What `obs_source_get_balance_value()` answers.
    pub balance: f32,
    /// `flags`.
    pub flags: u32,
    /// What the filters do: 0 the packet goes on, 1 dropped, 2 replaced.
    pub filter_mode: i32,
    /// `async_compensation`.
    pub compensation: bool,
    /// `audio_resampler_create()` answers NULL.
    pub create_fails: bool,
    /// `destroying`.
    pub destroying: bool,
}

impl Default for Init {
    fn default() -> Self {
        Self {
            balance: 0.5,
            flags: 0,
            filter_mode: 0,
            compensation: false,
            create_fails: false,
            destroying: false,
        }
    }
}

unsafe extern "C" {
    /// A test source. Free it with [`oracle_audio_input_path_free`].
    pub fn oracle_audio_input_path_new(init: *const Init) -> *mut c_void;
    /// Frees a test source.
    pub fn oracle_audio_input_path_free(source: *mut c_void);
    /// The source the stand-ins log to, and the output they answer with, on
    /// this thread.
    pub fn oracle_audio_input_path_begin(
        source: *mut c_void,
        format: c_int,
        rate: u32,
        speakers: c_int,
    );
    /// A packet with every plane set. Free it with
    /// [`oracle_audio_input_path_packet_free`].
    pub fn oracle_audio_input_path_packet_new(
        format: c_int,
        speakers: c_int,
        rate: u32,
        frames: u32,
        timestamp: u64,
        value: f32,
    ) -> *mut c_void;
    /// Frees a packet.
    pub fn oracle_audio_input_path_packet_free(audio: *mut c_void);
    /// Sets `async_compensation`.
    pub fn oracle_audio_input_path_set_compensation(source: *mut c_void, value: bool);
    /// Sets what `obs_source_get_balance_value()` answers.
    pub fn oracle_audio_input_path_set_balance(source: *mut c_void, value: f32);
    /// The log, every field, a hash of the storage and whether the mutexes are
    /// free, as text. Returns its length.
    pub fn oracle_audio_input_path_state(
        source: *const c_void,
        out: *mut c_char,
        cap: usize,
    ) -> usize;
    /// The size, alignment and field offsets of `struct obs_source_audio`.
    pub fn oracle_audio_input_path_layout(which: usize) -> usize;

    /// `obs_source_output_audio`.
    pub fn oracle_obs_source_output_audio(source: *mut c_void, audio_in: *const c_void);
}
