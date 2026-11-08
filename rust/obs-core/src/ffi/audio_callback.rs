//! C ABI for `libobs/obs-audio.c.h`, the audio tick: `audio_callback()`,
//! which `obs_reset_audio2()` hands the audio output. It is declared in
//! `obs-internal.h` without `EXPORT`, so it stays hidden (`rust-hidden.d`).
//!
//! The tick is the safe core's ([`crate::audio_callback`]), run on
//! [`Seam`], which implements its [`Tick`] over the core-audio seam
//! (`libobs/obs-audio-seam.h`, still C in `libobs.core-output`): one seam
//! call per field, lock and container call. `struct obs_core` and `struct
//! obs_source` are never mirrored: the fields stay in C, so ThreadSanitizer
//! keeps seeing their reads and writes. The decisions that change a
//! source's input or the core's buffering run through the C ABI of their
//! own ports (`obs_audio_discard_*`, `obs_audio_buffering_*`,
//! `obs_audio_mix_add_mixers`) on the pointers the seam gives, as the C
//! does.
//!
//! The calls into the rest of libobs (`obs_source_audio_render()`,
//! `obs_source_enum_active_tree()`, the source reference, name, removed and
//! active calls, the output's rate and channels, and `blog()`) go through
//! [`Calls`], so the parity test hands both sides the same recording
//! stand-ins. [`Libobs`] is libobs's own.

use std::ffi::{c_char, c_int, c_void};
use std::marker::PhantomData;
use std::ptr;

use crate::audio_callback::{self as core, Ignored, Tick};
use crate::audio_mix::{AUDIO_OUTPUT_FRAMES, MAX_AUDIO_CHANNELS, MAX_AUDIO_MIXES, Step, TsInfo};
use crate::ffi::audio_discard::{
    obs_audio_discard_ignore, obs_audio_discard_tick, obs_audio_input,
};
use crate::ffi::audio_mix::{
    audio_output_data, obs_audio_buffering, obs_audio_buffering_add, obs_audio_buffering_fixed,
    obs_audio_buffering_maxed, obs_audio_buffering_step, obs_audio_mix_add_mixers, ts_info,
};

/// `LOG_WARNING`, `LOG_INFO` and `LOG_DEBUG` in `util/base.h`.
const LOG_WARNING: c_int = 200;
const LOG_INFO: c_int = 300;
const LOG_DEBUG: c_int = 400;

/// `DARRAY_INVALID`.
const DARRAY_INVALID: usize = usize::MAX;

/// One row of `audio_output_buf`: `obs_audio_seam_output_row_t`.
type OutputRow = [*mut f32; MAX_AUDIO_CHANNELS];

/// `struct obs_task_info`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct obs_task_info {
    /// `obs_task_t task`.
    pub task: Option<unsafe extern "C" fn(param: *mut c_void)>,
    /// `void *param`.
    pub param: *mut c_void,
}

/// `obs_source_enum_proc_t`.
pub type EnumProc =
    unsafe extern "C" fn(parent: *mut c_void, child: *mut c_void, param: *mut c_void);

/// `blog()`'s type.
pub type Blog = unsafe extern "C" fn(level: c_int, format: *const c_char, ...);

unsafe extern "C" {
    fn obs_audio_seam_get_output() -> *mut c_void;
    fn obs_audio_seam_render_order_num() -> usize;
    fn obs_audio_seam_render_order_get(idx: usize) -> *mut c_void;
    fn obs_audio_seam_render_order_find(source: *mut c_void) -> usize;
    fn obs_audio_seam_render_order_push(source: *mut c_void);
    fn obs_audio_seam_render_order_clear();
    fn obs_audio_seam_root_nodes_num() -> usize;
    fn obs_audio_seam_root_nodes_get(idx: usize) -> *mut c_void;
    fn obs_audio_seam_root_nodes_push(source: *mut c_void);
    fn obs_audio_seam_root_nodes_clear();
    fn obs_audio_seam_buffered_timestamps_push_back(ts: *const ts_info);
    fn obs_audio_seam_buffered_timestamps_peek_front(ts: *mut ts_info);
    fn obs_audio_seam_buffered_timestamps_pop_front();
    fn obs_audio_seam_get_buffering() -> obs_audio_buffering;
    fn obs_audio_seam_get_fixed_buffer() -> bool;
    fn obs_audio_seam_get_buffering_wait_ticks() -> u64;
    fn obs_audio_seam_set_buffering_wait_ticks(value: u64);
    fn obs_audio_seam_get_monitoring_duplicating_source() -> *mut c_void;
    fn obs_audio_seam_lock_tasks() -> c_int;
    fn obs_audio_seam_unlock_tasks() -> c_int;
    fn obs_audio_seam_tasks_size() -> usize;
    fn obs_audio_seam_tasks_pop_front(info: *mut obs_task_info);

    fn obs_audio_seam_get_first_audio_source() -> *mut c_void;
    fn obs_audio_seam_lock_audio_sources() -> c_int;
    fn obs_audio_seam_unlock_audio_sources() -> c_int;
    fn obs_audio_seam_lock_mixes() -> c_int;
    fn obs_audio_seam_unlock_mixes() -> c_int;
    fn obs_audio_seam_mixes_num() -> usize;
    fn obs_audio_seam_mix_get_view(idx: usize) -> *mut c_void;
    fn obs_audio_seam_mix_get_mix_audio(idx: usize) -> bool;
    fn obs_audio_seam_view_lock_channels(view: *mut c_void) -> c_int;
    fn obs_audio_seam_view_unlock_channels(view: *mut c_void) -> c_int;
    fn obs_audio_seam_view_get_channel(view: *const c_void, idx: u32) -> *mut c_void;

    fn obs_audio_seam_source_get_next(source: *const c_void) -> *mut c_void;
    fn obs_audio_seam_source_lock_buf(source: *mut c_void) -> c_int;
    fn obs_audio_seam_source_unlock_buf(source: *mut c_void) -> c_int;
    fn obs_audio_seam_source_get_is_duplicated(source: *const c_void) -> bool;
    fn obs_audio_seam_source_set_is_duplicated(source: *mut c_void, value: bool);
    fn obs_audio_seam_source_get_pending(source: *const c_void) -> bool;
    fn obs_audio_seam_source_set_pending(source: *mut c_void, value: bool);
    fn obs_audio_seam_source_get_audio_ts(source: *const c_void) -> u64;
    fn obs_audio_seam_source_get_input_size(source: *const c_void) -> usize;
    fn obs_audio_seam_source_get_input(source: *mut c_void) -> obs_audio_input;
    fn obs_audio_seam_source_get_type(source: *const c_void) -> c_int;
    fn obs_audio_seam_source_get_output_flags(source: *const c_void) -> u32;
    fn obs_audio_seam_source_has_audio_render(source: *const c_void) -> bool;
    fn obs_audio_seam_source_get_output_buf(source: *mut c_void) -> *mut OutputRow;
    fn obs_audio_seam_source_get_muted(source: *const c_void) -> bool;
    fn obs_audio_seam_source_get_volume(source: *const c_void) -> f32;
    fn obs_audio_seam_source_get_audio_mixers(source: *const c_void) -> u32;
    fn obs_audio_seam_source_get_monitoring_enabled(source: *mut c_void) -> bool;
}

/// The calls the tick makes into the rest of libobs, one C function each.
///
/// # Safety
/// Each method is the C function its doc names, called with the pointers
/// the C passes: live sources and the core's live audio output.
pub unsafe trait Calls {
    /// `audio_output_get_sample_rate(audio)`.
    ///
    /// # Safety
    /// `audio` is the core's live audio output.
    unsafe fn sample_rate(audio: *const c_void) -> u32;
    /// `audio_output_get_channels(audio)`.
    ///
    /// # Safety
    /// As [`Calls::sample_rate`].
    unsafe fn channels(audio: *const c_void) -> usize;
    /// `obs_source_get_ref(source)`.
    ///
    /// # Safety
    /// `source` is a live `obs_source_t`.
    unsafe fn get_ref(source: *mut c_void) -> *mut c_void;
    /// `obs_source_release(source)`.
    ///
    /// # Safety
    /// `source` is a live `obs_source_t` the caller holds a reference on.
    unsafe fn release(source: *mut c_void);
    /// `obs_source_get_name(source)`.
    ///
    /// # Safety
    /// `source` is a live `obs_source_t`.
    unsafe fn get_name(source: *const c_void) -> *const c_char;
    /// `obs_source_removed(source)`.
    ///
    /// # Safety
    /// As [`Calls::get_name`].
    unsafe fn removed(source: *const c_void) -> bool;
    /// `obs_source_active(source)`.
    ///
    /// # Safety
    /// As [`Calls::get_name`].
    unsafe fn active(source: *const c_void) -> bool;
    /// `obs_source_audio_render(source, mixers, channels, sample_rate,
    /// size)`.
    ///
    /// # Safety
    /// As [`Calls::get_name`]; the caller holds a reference on it.
    unsafe fn audio_render(
        source: *mut c_void,
        mixers: u32,
        channels: usize,
        sample_rate: usize,
        size: usize,
    );
    /// `obs_source_enum_active_tree(source, callback, param)`.
    ///
    /// # Safety
    /// As [`Calls::get_name`]; `callback` takes `param`.
    unsafe fn enum_active_tree(source: *mut c_void, callback: EnumProc, param: *mut c_void);
    /// `blog`.
    fn blog() -> Blog;
}

/// [`Calls`] through libobs's own functions.
pub enum Libobs {}

unsafe extern "C" {
    fn audio_output_get_sample_rate(audio: *const c_void) -> u32;
    fn audio_output_get_channels(audio: *const c_void) -> usize;
    fn obs_source_get_ref(source: *mut c_void) -> *mut c_void;
    fn obs_source_release(source: *mut c_void);
    fn obs_source_get_name(source: *const c_void) -> *const c_char;
    fn obs_source_removed(source: *const c_void) -> bool;
    fn obs_source_active(source: *const c_void) -> bool;
    fn obs_source_audio_render(
        source: *mut c_void,
        mixers: u32,
        channels: usize,
        sample_rate: usize,
        size: usize,
    );
    fn obs_source_enum_active_tree(source: *mut c_void, callback: EnumProc, param: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

// SAFETY: each method is the one libobs function of its name, called with
// the caller's pointers.
unsafe impl Calls for Libobs {
    unsafe fn sample_rate(audio: *const c_void) -> u32 {
        // SAFETY: see the trait.
        unsafe { audio_output_get_sample_rate(audio) }
    }
    unsafe fn channels(audio: *const c_void) -> usize {
        // SAFETY: see the trait.
        unsafe { audio_output_get_channels(audio) }
    }
    unsafe fn get_ref(source: *mut c_void) -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { obs_source_get_ref(source) }
    }
    unsafe fn release(source: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { obs_source_release(source) }
    }
    unsafe fn get_name(source: *const c_void) -> *const c_char {
        // SAFETY: see the trait.
        unsafe { obs_source_get_name(source) }
    }
    unsafe fn removed(source: *const c_void) -> bool {
        // SAFETY: see the trait.
        unsafe { obs_source_removed(source) }
    }
    unsafe fn active(source: *const c_void) -> bool {
        // SAFETY: see the trait.
        unsafe { obs_source_active(source) }
    }
    unsafe fn audio_render(
        source: *mut c_void,
        mixers: u32,
        channels: usize,
        sample_rate: usize,
        size: usize,
    ) {
        // SAFETY: see the trait.
        unsafe { obs_source_audio_render(source, mixers, channels, sample_rate, size) }
    }
    unsafe fn enum_active_tree(source: *mut c_void, callback: EnumProc, param: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { obs_source_enum_active_tree(source, callback, param) }
    }
    fn blog() -> Blog {
        blog
    }
}

/// A non-NULL `obs_source_t *`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Source(*mut c_void);

/// What `obs_source_get_name()` gave: a name or NULL, only handed to
/// `blog()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Name(*const c_char);

/// A non-NULL `struct obs_view *`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct View(*mut c_void);

fn source(ptr: *mut c_void) -> Option<Source> {
    (!ptr.is_null()).then_some(Source(ptr))
}

impl From<obs_audio_buffering_step> for Step {
    fn from(step: obs_audio_buffering_step) -> Self {
        Self {
            added: step.added,
            max_reached: step.max_reached,
            ticks: step.ticks,
            ms: step.ms,
            total_ms: step.total_ms,
        }
    }
}

/// The tick's mixes and the libobs calls `L` makes for it.
pub struct Seam<L> {
    mixes: *mut audio_output_data,
    out_ts: *mut u64,
    calls: PhantomData<L>,
}

/// `push_audio_tree2`, as `obs_source_enum_active_tree()` calls it, with
/// the tick's [`Seam`] as `param`.
///
/// # Safety
/// `param` is the `Seam<L>` [`Tick::enum_active_tree`] passed, and `child`
/// a live source of the tree.
unsafe extern "C" fn push_audio_tree2<L: Calls>(
    _parent: *mut c_void,
    child: *mut c_void,
    param: *mut c_void,
) {
    // SAFETY: the tick's seam, which nothing else uses during the
    // enumeration (the contract).
    let seam = unsafe { &mut *param.cast::<Seam<L>>() };
    if let Some(child) = source(child) {
        core::push_tree2(seam, child);
    }
}

// SAFETY (every method): each source is one the C caller or the seam gave
// (live while the render order, the list or a view holds it), each call is
// the one seam accessor, lock call, port or libobs function that names it,
// and the core holds the locks the seam header asks for.
impl<L: Calls> Tick for Seam<L> {
    type Source = Source;
    type View = View;
    type Name = Name;
    type Task = obs_task_info;

    fn sample_rate(&mut self) -> usize {
        // SAFETY: see the impl; the core has its audio while it ticks.
        unsafe { L::sample_rate(obs_audio_seam_get_output()) as usize }
    }
    fn channels(&mut self) -> usize {
        // SAFETY: as above.
        unsafe { L::channels(obs_audio_seam_get_output()) }
    }
    fn render_order_len(&mut self) -> usize {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_render_order_num() }
    }
    fn render_order_get(&mut self, idx: usize) -> Source {
        // SAFETY: see the impl; `idx` is below the length.
        Source(unsafe { obs_audio_seam_render_order_get(idx) })
    }
    fn render_order_find(&mut self, source: Source) -> Option<usize> {
        // SAFETY: see the impl.
        let idx = unsafe { obs_audio_seam_render_order_find(source.0) };
        (idx != DARRAY_INVALID).then_some(idx)
    }
    fn render_order_push(&mut self, source: Source) {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_render_order_push(source.0) }
    }
    fn render_order_clear(&mut self) {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_render_order_clear() }
    }
    fn root_nodes_len(&mut self) -> usize {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_root_nodes_num() }
    }
    fn root_nodes_get(&mut self, idx: usize) -> Source {
        // SAFETY: see the impl; `idx` is below the length.
        Source(unsafe { obs_audio_seam_root_nodes_get(idx) })
    }
    fn root_nodes_push(&mut self, source: Source) {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_root_nodes_push(source.0) }
    }
    fn root_nodes_clear(&mut self) {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_root_nodes_clear() }
    }
    fn timestamps_push_back(&mut self, ts: TsInfo) {
        let ts = ts_info::from(ts);
        // SAFETY: see the impl; one whole entry.
        unsafe { obs_audio_seam_buffered_timestamps_push_back(&raw const ts) }
    }
    fn timestamps_peek_front(&mut self) -> TsInfo {
        let mut ts = ts_info { start: 0, end: 0 };
        // SAFETY: see the impl; the tick pushed one entry first.
        unsafe { obs_audio_seam_buffered_timestamps_peek_front(&raw mut ts) };
        ts.into()
    }
    fn timestamps_pop_front(&mut self) {
        // SAFETY: see the impl; the tick's own entry is still queued.
        unsafe { obs_audio_seam_buffered_timestamps_pop_front() }
    }
    fn buffering_maxed(&mut self) -> bool {
        // SAFETY: see the impl; the fields of the core.
        unsafe {
            let buffering = obs_audio_seam_get_buffering();
            obs_audio_buffering_maxed(&raw const buffering)
        }
    }
    fn buffering_fixed(&mut self, sample_rate: usize, ts: &mut TsInfo) -> Step {
        let mut window = ts_info::from(*ts);
        // SAFETY: see the impl; the fields of the core, which only the
        // audio thread uses.
        let step = unsafe {
            let buffering = obs_audio_seam_get_buffering();
            obs_audio_buffering_fixed(&raw const buffering, sample_rate, &raw mut window)
        };
        *ts = window.into();
        step.into()
    }
    fn buffering_add(&mut self, sample_rate: usize, ts: &mut TsInfo, min_ts: u64) -> Step {
        let mut window = ts_info::from(*ts);
        // SAFETY: as above.
        let step = unsafe {
            let buffering = obs_audio_seam_get_buffering();
            obs_audio_buffering_add(&raw const buffering, sample_rate, &raw mut window, min_ts)
        };
        *ts = window.into();
        step.into()
    }
    fn fixed_buffer(&mut self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_get_fixed_buffer() }
    }
    fn wait_ticks(&mut self) -> u64 {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_get_buffering_wait_ticks() }
    }
    fn set_wait_ticks(&mut self, value: u64) {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_set_buffering_wait_ticks(value) }
    }
    fn set_out_ts(&mut self, value: u64) {
        // SAFETY: the caller's output (audio_callback()'s contract).
        unsafe { *self.out_ts = value };
    }
    fn monitoring_duplicating_source(&mut self) -> Option<Source> {
        // SAFETY: see the impl.
        source(unsafe { obs_audio_seam_get_monitoring_duplicating_source() })
    }
    fn lock_tasks(&mut self) {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_lock_tasks() };
    }
    fn unlock_tasks(&mut self) {
        // SAFETY: see the impl; the core locked it.
        unsafe { obs_audio_seam_unlock_tasks() };
    }
    fn tasks_size(&mut self) -> usize {
        // SAFETY: see the impl; under task_mutex.
        unsafe { obs_audio_seam_tasks_size() }
    }
    fn pop_task(&mut self) -> obs_task_info {
        let mut info = obs_task_info {
            task: None,
            param: ptr::null_mut(),
        };
        // SAFETY: see the impl; under task_mutex, with a task queued.
        unsafe { obs_audio_seam_tasks_pop_front(&raw mut info) };
        info
    }
    fn run_task(&mut self, task: obs_task_info) {
        if let Some(run) = task.task {
            // SAFETY: the task and the parameter it was queued with.
            unsafe { run(task.param) };
        }
    }
    fn first_audio_source(&mut self) -> Option<Source> {
        // SAFETY: see the impl; under audio_sources_mutex.
        source(unsafe { obs_audio_seam_get_first_audio_source() })
    }
    fn lock_audio_sources(&mut self) {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_lock_audio_sources() };
    }
    fn unlock_audio_sources(&mut self) {
        // SAFETY: see the impl; the core locked it.
        unsafe { obs_audio_seam_unlock_audio_sources() };
    }
    fn lock_mixes(&mut self) {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_lock_mixes() };
    }
    fn unlock_mixes(&mut self) {
        // SAFETY: see the impl; the core locked it.
        unsafe { obs_audio_seam_unlock_mixes() };
    }
    fn mixes_len(&mut self) -> usize {
        // SAFETY: see the impl; under mixes_mutex.
        unsafe { obs_audio_seam_mixes_num() }
    }
    fn mix_view(&mut self, idx: usize) -> Option<View> {
        // SAFETY: see the impl; under mixes_mutex, `idx` below the count.
        let view = unsafe { obs_audio_seam_mix_get_view(idx) };
        (!view.is_null()).then_some(View(view))
    }
    fn mix_audio(&mut self, idx: usize) -> bool {
        // SAFETY: as above.
        unsafe { obs_audio_seam_mix_get_mix_audio(idx) }
    }
    fn lock_channels(&mut self, view: View) {
        // SAFETY: see the impl; a view of a mix, under mixes_mutex.
        unsafe { obs_audio_seam_view_lock_channels(view.0) };
    }
    fn unlock_channels(&mut self, view: View) {
        // SAFETY: as above; the core locked it.
        unsafe { obs_audio_seam_view_unlock_channels(view.0) };
    }
    fn channel(&mut self, view: View, idx: u32) -> Option<Source> {
        // SAFETY: as above, under its channels_mutex; `idx` below
        // MAX_CHANNELS.
        source(unsafe { obs_audio_seam_view_get_channel(view.0, idx) })
    }
    fn next_audio_source(&mut self, source: Source) -> Option<Source> {
        // SAFETY: see the impl; under audio_sources_mutex.
        self::source(unsafe { obs_audio_seam_source_get_next(source.0) })
    }
    fn lock_buf(&mut self, source: Source) {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_source_lock_buf(source.0) };
    }
    fn unlock_buf(&mut self, source: Source) {
        // SAFETY: see the impl; the core locked it.
        unsafe { obs_audio_seam_source_unlock_buf(source.0) };
    }
    fn is_duplicated(&mut self, source: Source) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_source_get_is_duplicated(source.0) }
    }
    fn set_is_duplicated(&mut self, source: Source, value: bool) {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_source_set_is_duplicated(source.0, value) }
    }
    fn pending(&mut self, source: Source) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_source_get_pending(source.0) }
    }
    fn set_pending(&mut self, source: Source, value: bool) {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_source_set_pending(source.0, value) }
    }
    fn audio_ts(&mut self, source: Source) -> u64 {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_source_get_audio_ts(source.0) }
    }
    fn input_size(&mut self, source: Source) -> usize {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_source_get_input_size(source.0) }
    }
    fn kind(&mut self, source: Source) -> u32 {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_source_get_type(source.0) }.cast_unsigned()
    }
    fn output_flags(&mut self, source: Source) -> u32 {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_source_get_output_flags(source.0) }
    }
    fn has_audio_render(&mut self, source: Source) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_source_has_audio_render(source.0) }
    }
    fn has_output_buf(&mut self, source: Source) -> bool {
        // SAFETY: see the impl; the source's array of MAX_AUDIO_MIXES rows.
        unsafe { !(*obs_audio_seam_source_get_output_buf(source.0))[0].is_null() }
    }
    fn muted(&mut self, source: Source) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_source_get_muted(source.0) }
    }
    fn volume(&mut self, source: Source) -> f32 {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_source_get_volume(source.0) }
    }
    fn audio_mixers(&mut self, source: Source) -> u32 {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_source_get_audio_mixers(source.0) }
    }
    fn monitoring_enabled(&mut self, source: Source) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_audio_seam_source_get_monitoring_enabled(source.0) }
    }
    fn clear_output(&mut self, source: Source, mixes: u32) {
        // SAFETY: see the impl; the array of MAX_AUDIO_MIXES rows, each
        // buffer NULL or AUDIO_OUTPUT_FRAMES floats.
        unsafe {
            let rows = obs_audio_seam_source_get_output_buf(source.0);
            for mix in 0..MAX_AUDIO_MIXES {
                if mixes & (1u32 << mix) == 0 {
                    continue;
                }
                let row = *rows.add(mix);
                for buf in row {
                    if !buf.is_null() {
                        ptr::write_bytes(buf, 0, AUDIO_OUTPUT_FRAMES);
                    }
                }
            }
        }
    }
    fn ignore(
        &mut self,
        source: Source,
        channels: usize,
        sample_rate: usize,
        start_ts: u64,
    ) -> Ignored {
        // SAFETY: see the impl; the input fields of the source, under its
        // audio_buf_mutex.
        let step = unsafe {
            let input = obs_audio_seam_source_get_input(source.0);
            obs_audio_discard_ignore(&raw const input, channels, sample_rate, start_ts)
        };
        Ignored {
            rerender: step.rerender,
            no_timestamp: step.no_timestamp,
            buffered: step.buffered,
            lag: step.lag,
            warn: step.warn,
            warn_lag: step.warn_lag,
        }
    }
    fn discard_tick(
        &mut self,
        source: Source,
        audio_render: bool,
        channels: usize,
        sample_rate: usize,
        ts: TsInfo,
    ) {
        let ts = ts_info::from(ts);
        // SAFETY: as above.
        unsafe {
            let input = obs_audio_seam_source_get_input(source.0);
            obs_audio_discard_tick(
                &raw const input,
                audio_render,
                channels,
                sample_rate,
                &raw const ts,
            );
        }
    }
    fn mix(
        &mut self,
        source: Source,
        channels: usize,
        sample_rate: usize,
        ts: TsInfo,
        mixers: u32,
    ) {
        let ts = ts_info::from(ts);
        // SAFETY: see the impl; the tick's mixes and the source's output,
        // under its audio_buf_mutex.
        unsafe {
            let buf = obs_audio_seam_source_get_output_buf(source.0);
            let audio_ts = obs_audio_seam_source_get_audio_ts(source.0);
            obs_audio_mix_add_mixers(
                self.mixes,
                buf,
                audio_ts,
                &raw const ts,
                channels,
                sample_rate,
                mixers,
            );
        }
    }
    fn get_ref(&mut self, source: Source) -> Option<Source> {
        // SAFETY: see the impl.
        self::source(unsafe { L::get_ref(source.0) })
    }
    fn release(&mut self, source: Source) {
        // SAFETY: see the impl; the reference the render order took.
        unsafe { L::release(source.0) }
    }
    fn name(&mut self, source: Source) -> Name {
        // SAFETY: see the impl.
        Name(unsafe { L::get_name(source.0) })
    }
    fn removed(&mut self, source: Source) -> bool {
        // SAFETY: see the impl.
        unsafe { L::removed(source.0) }
    }
    fn active(&mut self, source: Source) -> bool {
        // SAFETY: see the impl.
        unsafe { L::active(source.0) }
    }
    fn audio_render(
        &mut self,
        source: Source,
        mixers: u32,
        channels: usize,
        sample_rate: usize,
        size: usize,
    ) {
        // SAFETY: see the impl; a source of the render order.
        unsafe { L::audio_render(source.0, mixers, channels, sample_rate, size) }
    }
    fn enum_active_tree(&mut self, source: Source) {
        let param = ptr::from_mut(self).cast::<c_void>();
        // SAFETY: see the impl; the callback gets this seam back as
        // `param`, and the tick uses nothing else until it returns.
        unsafe { L::enum_active_tree(source.0, push_audio_tree2::<L>, param) }
    }
    fn log_backwards(&mut self, name: Name) {
        // SAFETY: a static format and the arguments it names.
        unsafe {
            L::blog()(
                LOG_DEBUG,
                c"render audio source %s timestamp has gone backwards".as_ptr(),
                name.0,
            );
        }
    }
    fn log_ts_lag(&mut self, name: Name, lag: u64) {
        // SAFETY: as above.
        unsafe {
            L::blog()(
                LOG_DEBUG,
                c"[src: %s] ts lag after ignoring: %llu".as_ptr(),
                name.0,
                lag,
            );
        }
    }
    #[allow(clippy::cast_precision_loss)]
    fn log_lagging(&mut self, name: Name, lag: u64) {
        // SAFETY: as above.
        unsafe {
            L::blog()(
                LOG_WARNING,
                c"Source %s audio is lagging (over by %.02f ms) at max audio buffering. Restarting source audio."
                    .as_ptr(),
                name.0,
                lag as f64 / 1_000_000.,
            );
        }
    }
    fn log_fixed(&mut self, total_ms: i32) {
        // SAFETY: as above.
        unsafe {
            L::blog()(
                LOG_INFO,
                c"Enabling fixed audio buffering, total audio buffering is now %d milliseconds"
                    .as_ptr(),
                total_ms,
            );
        }
    }
    fn log_max_reached(&mut self) {
        // SAFETY: as above.
        unsafe { L::blog()(LOG_WARNING, c"Max audio buffering reached!".as_ptr()) }
    }
    fn log_adding(&mut self, ms: i32, total_ms: i32, name: Option<Name>) {
        // SAFETY: as above; NULL for no name, as the C passes.
        unsafe {
            L::blog()(
                LOG_INFO,
                c"adding %d milliseconds of audio buffering, total audio buffering is now %d milliseconds (source: %s)\n"
                    .as_ptr(),
                ms,
                total_ms,
                name.map_or(ptr::null(), |n| n.0),
            );
        }
    }
}

/// `audio_callback()` with the libobs calls of `L`.
///
/// # Safety
/// As [`audio_callback`].
pub unsafe fn audio_callback_with<L: Calls>(
    start_ts_in: u64,
    end_ts_in: u64,
    out_ts: *mut u64,
    mixers: u32,
    mixes: *mut audio_output_data,
) -> bool {
    let mut seam = Seam::<L> {
        mixes,
        out_ts,
        calls: PhantomData,
    };
    // The core writes `*out_ts` through the seam where the C does.
    let (result, _) = core::audio_callback(&mut seam, start_ts_in, end_ts_in, mixers);
    result
}

/// `audio_callback()`: one audio tick, which the audio output's thread
/// calls with its mixes cleared.
///
/// # Safety
/// libobs's core is up with its audio; `out_ts` is valid; `mixes` points to
/// `MAX_AUDIO_MIXES` `struct audio_output_data` whose channels are NULL or
/// `AUDIO_OUTPUT_FRAMES` floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audio_callback(
    param: *mut c_void,
    start_ts_in: u64,
    end_ts_in: u64,
    out_ts: *mut u64,
    mixers: u32,
    mixes: *mut audio_output_data,
) -> bool {
    let _ = param;
    // SAFETY: the caller's contract.
    unsafe { audio_callback_with::<Libobs>(start_ts_in, end_ts_in, out_ts, mixers, mixes) }
}
