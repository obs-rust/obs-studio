//! C ABI for `libobs/obs-source-audio-input-path.c.h`, the path that takes one
//! audio packet from a source. `obs_source_output_audio()` is `EXPORT` in `obs.h`,
//! so plugins call this shim (`rust-exports.d`).
//!
//! The function is the safe core's (`obs_source::audio_input_path`), run on
//! [`Seam`], which implements its `Source` over
//! `libobs/obs-source-audio-input-path-seam.h`, still C in `libobs.core-source`:
//! one seam call per method. `struct obs_source` is never mirrored, the fields
//! stay in C, so ThreadSanitizer keeps seeing their reads and writes, and the
//! samples stay in C. `struct obs_source_audio` is public ABI, so the shim reads
//! it as `obs_source::audio_input_path::Audio`. The C seam checks that layout at
//! compile time, and the parity test checks it against the C. The locking is the
//! C's.

use std::ffi::{c_char, c_int, c_void};
use std::ptr::{self, NonNull};

use obs_source::audio_input::Format;
use obs_source::audio_input_path::{self as core, Audio, MAX_PLANES, Planes, Resampled, Source};

/// `struct obs_source_audio_input_format`.
#[repr(C)]
#[derive(Clone, Copy)]
struct CFormat {
    format: c_int,
    samples_per_sec: u32,
    speakers: c_int,
}

impl From<CFormat> for Format {
    fn from(value: CFormat) -> Self {
        Self {
            format: value.format,
            samples_per_sec: value.samples_per_sec,
            speakers: value.speakers,
        }
    }
}

impl From<Format> for CFormat {
    fn from(value: Format) -> Self {
        Self {
            format: value.format,
            samples_per_sec: value.samples_per_sec,
            speakers: value.speakers,
        }
    }
}

unsafe extern "C" {
    fn obs_source_audio_input_path_seam_valid(source: *const c_void) -> bool;
    fn obs_source_audio_input_path_seam_destroying(source: *const c_void) -> bool;
    fn obs_source_audio_input_path_seam_input_valid(audio_in: *const c_void) -> bool;
    fn obs_source_audio_input_path_seam_audio_planes(format: c_int, speakers: c_int) -> usize;

    fn obs_source_audio_input_path_seam_output_format() -> CFormat;
    fn obs_source_audio_input_path_seam_planes() -> usize;
    fn obs_source_audio_input_path_seam_block_size() -> usize;
    fn obs_source_audio_input_path_seam_channels() -> usize;

    fn obs_source_audio_input_path_seam_lock_audio_buf(source: *mut c_void);
    fn obs_source_audio_input_path_seam_unlock_audio_buf(source: *mut c_void);
    fn obs_source_audio_input_path_seam_lock_filter(source: *mut c_void);
    fn obs_source_audio_input_path_seam_unlock_filter(source: *mut c_void);
    fn obs_source_audio_input_path_seam_lock_audio(source: *mut c_void);
    fn obs_source_audio_input_path_seam_unlock_audio(source: *mut c_void);

    fn obs_source_audio_input_path_seam_has_resampler(source: *const c_void) -> bool;
    fn obs_source_audio_input_path_seam_get_next_resample_in_ts(source: *const c_void) -> u64;
    fn obs_source_audio_input_path_seam_set_next_resample_in_ts(source: *mut c_void, value: u64);
    fn obs_source_audio_input_path_seam_get_sample_info(source: *const c_void) -> CFormat;
    fn obs_source_audio_input_path_seam_get_sample_speakers(source: *const c_void) -> c_int;
    fn obs_source_audio_input_path_seam_set_sample_info(source: *mut c_void, value: CFormat);
    fn obs_source_audio_input_path_seam_get_last_async_compensation(source: *const c_void) -> bool;
    fn obs_source_audio_input_path_seam_set_last_async_compensation(
        source: *mut c_void,
        value: bool,
    );
    fn obs_source_audio_input_path_seam_get_async_compensation(source: *const c_void) -> bool;
    fn obs_source_audio_input_path_seam_get_audio_failed(source: *const c_void) -> bool;
    fn obs_source_audio_input_path_seam_set_audio_failed(source: *mut c_void, value: bool);
    fn obs_source_audio_input_path_seam_get_resample_offset(source: *const c_void) -> u64;
    fn obs_source_audio_input_path_seam_set_resample_offset(source: *mut c_void, value: u64);
    fn obs_source_audio_input_path_seam_get_audio_storage_size(source: *const c_void) -> usize;
    fn obs_source_audio_input_path_seam_set_audio_storage_size(source: *mut c_void, value: usize);
    fn obs_source_audio_input_path_seam_set_data_frames(source: *mut c_void, value: u32);
    fn obs_source_audio_input_path_seam_set_data_timestamp(source: *mut c_void, value: u64);
    fn obs_source_audio_input_path_seam_get_flags(source: *const c_void) -> u32;

    fn obs_source_audio_input_path_seam_destroy_resampler(source: *mut c_void);
    fn obs_source_audio_input_path_seam_create_resampler(
        source: *mut c_void,
        output: CFormat,
    ) -> bool;
    fn obs_source_audio_input_path_seam_log_resampler_failed();
    fn obs_source_audio_input_path_seam_resample(
        source: *mut c_void,
        output: *mut Option<NonNull<u8>>,
        out_frames: *mut u32,
        offset: *mut u64,
        data: *const Option<NonNull<u8>>,
        frames: u32,
    );

    fn obs_source_audio_input_path_seam_free_storage(source: *mut c_void, plane: usize);
    fn obs_source_audio_input_path_seam_alloc_storage(
        source: *mut c_void,
        plane: usize,
        size: usize,
    );
    fn obs_source_audio_input_path_seam_copy_to_storage(
        source: *mut c_void,
        plane: usize,
        data: *const c_char,
        size: usize,
    );

    fn obs_source_audio_input_path_seam_balance_value(source: *const c_void) -> f32;
    fn obs_source_audio_input_path_seam_apply_balance(
        source: *mut c_void,
        frames: u32,
        balance: f32,
    );
    fn obs_source_audio_input_path_seam_downmix(source: *mut c_void, channels: usize, frames: u32);

    fn obs_source_audio_input_path_seam_filter_audio(source: *mut c_void) -> *mut c_void;
    fn obs_source_audio_input_path_seam_output_audio_data(
        source: *mut c_void,
        output: *const c_void,
    );
}

/// The source a shim was handed. The core checks it for NULL first, and the
/// C seam's check is the only method that sees a NULL.
struct Seam(*mut c_void);

// SAFETY (every method): `self.0` is the live source the C caller passed, each
// call is the one seam function that names it, and the core holds the locks
// the seam header asks for.
impl Source for Seam {
    type Plane = NonNull<u8>;
    type Input = *const c_void;
    type Output = NonNull<c_void>;

    fn valid(&self) -> bool {
        // SAFETY: this only compares the pointer with NULL.
        unsafe { obs_source_audio_input_path_seam_valid(self.0) }
    }
    fn destroying(&self) -> bool {
        // SAFETY: see the impl; `valid()` answered true.
        unsafe { obs_source_audio_input_path_seam_destroying(self.0) }
    }
    fn input_valid(&self, input: *const c_void) -> bool {
        // SAFETY: this only compares the pointer with NULL.
        unsafe { obs_source_audio_input_path_seam_input_valid(input) }
    }
    fn read_input(&self, input: *const c_void) -> Audio<NonNull<u8>> {
        // SAFETY: `input_valid()` answered true, so the caller's pointer is a
        // live `struct obs_source_audio`, which has this layout (C seam).
        unsafe { input.cast::<Audio<NonNull<u8>>>().read() }
    }
    fn audio_planes(&self, format: i32, speakers: i32) -> usize {
        // SAFETY: plain values.
        unsafe { obs_source_audio_input_path_seam_audio_planes(format, speakers) }
    }
    fn output_format(&self) -> Format {
        // SAFETY: no arguments; libobs has its audio while a source runs.
        unsafe { obs_source_audio_input_path_seam_output_format() }.into()
    }
    fn planes(&self) -> usize {
        // SAFETY: as for `output_format`.
        unsafe { obs_source_audio_input_path_seam_planes() }
    }
    fn block_size(&self) -> usize {
        // SAFETY: as for `output_format`.
        unsafe { obs_source_audio_input_path_seam_block_size() }
    }
    fn channels(&self) -> usize {
        // SAFETY: as for `output_format`.
        unsafe { obs_source_audio_input_path_seam_channels() }
    }
    fn lock_audio_buf(&self) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_input_path_seam_lock_audio_buf(self.0) }
    }
    fn unlock_audio_buf(&self) {
        // SAFETY: see the impl; the core locked it.
        unsafe { obs_source_audio_input_path_seam_unlock_audio_buf(self.0) }
    }
    fn lock_filter(&self) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_input_path_seam_lock_filter(self.0) }
    }
    fn unlock_filter(&self) {
        // SAFETY: see the impl; the core locked it.
        unsafe { obs_source_audio_input_path_seam_unlock_filter(self.0) }
    }
    fn lock_audio(&self) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_input_path_seam_lock_audio(self.0) }
    }
    fn unlock_audio(&self) {
        // SAFETY: see the impl; the core locked it.
        unsafe { obs_source_audio_input_path_seam_unlock_audio(self.0) }
    }
    fn has_resampler(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_input_path_seam_has_resampler(self.0) }
    }
    fn next_resample_in_ts(&self) -> u64 {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_input_path_seam_get_next_resample_in_ts(self.0) }
    }
    fn set_next_resample_in_ts(&self, value: u64) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_input_path_seam_set_next_resample_in_ts(self.0, value) }
    }
    fn sample_info(&self) -> Format {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_input_path_seam_get_sample_info(self.0) }.into()
    }
    fn sample_speakers(&self) -> i32 {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_input_path_seam_get_sample_speakers(self.0) }
    }
    fn set_sample_info(&self, value: Format) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_input_path_seam_set_sample_info(self.0, value.into()) }
    }
    fn last_async_compensation(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_input_path_seam_get_last_async_compensation(self.0) }
    }
    fn set_last_async_compensation(&self, value: bool) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_input_path_seam_set_last_async_compensation(self.0, value) }
    }
    fn async_compensation(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_input_path_seam_get_async_compensation(self.0) }
    }
    fn audio_failed(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_input_path_seam_get_audio_failed(self.0) }
    }
    fn set_audio_failed(&self, value: bool) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_input_path_seam_set_audio_failed(self.0, value) }
    }
    fn resample_offset(&self) -> u64 {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_input_path_seam_get_resample_offset(self.0) }
    }
    fn set_resample_offset(&self, value: u64) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_input_path_seam_set_resample_offset(self.0, value) }
    }
    fn audio_storage_size(&self) -> usize {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_input_path_seam_get_audio_storage_size(self.0) }
    }
    fn set_audio_storage_size(&self, value: usize) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_input_path_seam_set_audio_storage_size(self.0, value) }
    }
    fn set_data_frames(&self, value: u32) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_input_path_seam_set_data_frames(self.0, value) }
    }
    fn set_data_timestamp(&self, value: u64) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_input_path_seam_set_data_timestamp(self.0, value) }
    }
    fn flags(&self) -> u32 {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_input_path_seam_get_flags(self.0) }
    }
    fn destroy_resampler(&self) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_input_path_seam_destroy_resampler(self.0) }
    }
    fn create_resampler(&self, output: Format) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_input_path_seam_create_resampler(self.0, output.into()) }
    }
    fn log_resampler_failed(&self) {
        // SAFETY: no arguments.
        unsafe { obs_source_audio_input_path_seam_log_resampler_failed() }
    }
    fn resample(
        &self,
        data: &Planes<NonNull<u8>>,
        frames: u32,
        offset: u64,
    ) -> Resampled<NonNull<u8>> {
        let mut planes = [None; MAX_PLANES];
        let mut out_frames = frames;
        let mut out_offset = offset;
        // SAFETY: see the impl; `has_resampler()` answered true. An array of
        // `Option<NonNull<u8>>` has the layout of `uint8_t *[MAX_AV_PLANES]`,
        // and the planes of `data` are the caller's.
        unsafe {
            obs_source_audio_input_path_seam_resample(
                self.0,
                planes.as_mut_ptr(),
                &raw mut out_frames,
                &raw mut out_offset,
                data.as_ptr(),
                frames,
            );
        }
        Resampled {
            planes,
            frames: out_frames,
            offset: out_offset,
        }
    }
    fn free_storage(&self, plane: usize) {
        // SAFETY: see the impl; `plane` is below the output's planes.
        unsafe { obs_source_audio_input_path_seam_free_storage(self.0, plane) }
    }
    fn alloc_storage(&self, plane: usize, size: usize) {
        // SAFETY: as for `free_storage`.
        unsafe { obs_source_audio_input_path_seam_alloc_storage(self.0, plane, size) }
    }
    fn copy_to_storage(&self, plane: usize, data: Option<NonNull<u8>>, size: usize) {
        let data = data.map_or(ptr::null(), |plane| plane.as_ptr().cast_const().cast());
        // SAFETY: as for `free_storage`; the storage holds `size` bytes, and the
        // caller's plane holds the packet, as in the C.
        unsafe { obs_source_audio_input_path_seam_copy_to_storage(self.0, plane, data, size) }
    }
    fn balance_value(&self) -> f32 {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_input_path_seam_balance_value(self.0) }
    }
    fn apply_balance(&self, frames: u32, balance: f32) {
        // SAFETY: see the impl; the storage holds `frames` samples.
        unsafe { obs_source_audio_input_path_seam_apply_balance(self.0, frames, balance) }
    }
    fn downmix(&self, channels: usize, frames: u32) {
        // SAFETY: as for `apply_balance`.
        unsafe { obs_source_audio_input_path_seam_downmix(self.0, channels, frames) }
    }
    fn filter_audio(&self) -> Option<NonNull<c_void>> {
        // SAFETY: see the impl.
        NonNull::new(unsafe { obs_source_audio_input_path_seam_filter_audio(self.0) })
    }
    fn output_audio_data(&self, output: NonNull<c_void>) {
        // SAFETY: see the impl; `output` is the packet the filters returned.
        unsafe {
            obs_source_audio_input_path_seam_output_audio_data(
                self.0,
                output.as_ptr().cast_const(),
            );
        }
    }
}

/// `void obs_source_output_audio(obs_source_t *source, const struct
/// obs_source_audio *audio_in)`.
///
/// # Safety
/// `source` is NULL or a live source. `audio_in` is NULL or a live packet with the
/// planes that its format and speakers say.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_output_audio(source: *mut c_void, audio_in: *const c_void) {
    core::output_audio(&Seam(source), audio_in);
}

/// The layout of `struct obs_source_audio` as the shim reads it, for the parity
/// test: size, alignment and each field's offset.
#[must_use]
pub fn audio_layout() -> [usize; 8] {
    [
        size_of::<Audio<NonNull<u8>>>(),
        align_of::<Audio<NonNull<u8>>>(),
        std::mem::offset_of!(Audio<NonNull<u8>>, data),
        std::mem::offset_of!(Audio<NonNull<u8>>, frames),
        std::mem::offset_of!(Audio<NonNull<u8>>, speakers),
        std::mem::offset_of!(Audio<NonNull<u8>>, format),
        std::mem::offset_of!(Audio<NonNull<u8>>, samples_per_sec),
        std::mem::offset_of!(Audio<NonNull<u8>>, timestamp),
    ]
}
