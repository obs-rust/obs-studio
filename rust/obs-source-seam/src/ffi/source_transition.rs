//! C ABI for the rest of `libobs/obs-source-transition.c.h`, the audio
//! render, init and free of a transition: `obs_transition_audio_render()`,
//! `EXPORT` through `obs.h` (`rust-exports.d`), and `obs_transition_init()`
//! and `obs_transition_free()`, which a source's create and destroy call,
//! declared in `obs-internal.h` without `EXPORT` (`rust-hidden.d`).
//!
//! The functions are the safe core's (`obs_source::transition_audio`), run
//! on the stop shims' [`Seam`], which implements that core's `Transition`
//! here (its `transition_stop::Transition` is the stop shims'): one
//! transition seam accessor (`libobs/obs-source-transition-seam.h`) per
//! field read or write, mutex call and child flag read (`audio_pending`,
//! `audio_is_duplicated`, `audio_output_buf[0][0]`), and the context data.
//! `struct obs_source` is never mirrored. The calls into the rest of libobs
//! (`obs_source_removed()`, `obs_source_get_audio_timestamp()`,
//! `obs_source_get_audio_mix()`, `obs_source_get_ref()`, the graphics
//! context `obs->video.graphics` through
//! `obs_source_transition_seam_get_graphics()`, `gs_texrender_create()`,
//! `gs_texrender_destroy()`, `gs_enter_context()` and `gs_leave_context()`)
//! go through [`Calls`], which extends the stop shims' calls: [`Libobs`]
//! calls them, and the parity test hands in the oracle's recording
//! stand-ins, as the C oracle calls them. The stop helpers the render asks
//! (`transition_calc_time()`, `transition_stop()` and
//! `transition_handle_stop()`) are the core's, called in Rust.
//!
//! The caller's mix and each child's are `struct obs_source_audio_mix`
//! ([`AudioMix`], laid out as `obs-source.h` and `media-io/audio-io.h` lay
//! it out): the samples are read and written through its `float *`s, and
//! slot A's copy is one `TOTAL_AUDIO_SIZE` copy from its
//! `audio_output_buf[0][0]` into `audio->output[0].data[0]`, as the C's
//! `memcpy()`. The type's mix callbacks are called through their pointers,
//! with the transition's context data, outside the lock.
//!
//! The locks are the C's: the audio render takes `transition_mutex` around
//! its reads of the slots, flags, clock and children and its stop, and
//! calls the callbacks and `obs_source_release()` after it lets go; init
//! and free take none.
//!
//! No intentional differences from C.

use std::ffi::c_void;
use std::ptr;

use obs_source::transition_audio::{self as core, MAX_AUDIO_MIXES, Request, Transition};

use super::source_transition_stop::{self as stop, ChildSource, Seam};

/// `MAX_AUDIO_CHANNELS` (`media-io/audio-io.h`).
pub const MAX_AUDIO_CHANNELS: usize = 8;

/// `AUDIO_OUTPUT_FRAMES` (`media-io/audio-io.h`).
pub const AUDIO_OUTPUT_FRAMES: usize = 1024;

/// The floats of `TOTAL_AUDIO_SIZE` (`media-io/audio-io.h`): every frame of
/// every channel of every mix.
pub const TOTAL_AUDIO_FLOATS: usize = MAX_AUDIO_MIXES * MAX_AUDIO_CHANNELS * AUDIO_OUTPUT_FRAMES;

/// `struct audio_output_data` (`media-io/audio-io.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AudioOutputData {
    /// `float *data[MAX_AUDIO_CHANNELS]`.
    pub data: [*mut f32; MAX_AUDIO_CHANNELS],
}

/// `struct obs_source_audio_mix` (`obs-source.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AudioMix {
    /// `struct audio_output_data output[MAX_AUDIO_MIXES]`.
    pub output: [AudioOutputData; MAX_AUDIO_MIXES],
}

impl AudioMix {
    /// A mix whose every channel is NULL, as the C's uninitialized
    /// `struct obs_source_audio_mix child_audio` is before
    /// `obs_source_get_audio_mix()` fills it.
    pub const fn empty() -> Self {
        Self {
            output: [AudioOutputData {
                data: [ptr::null_mut(); MAX_AUDIO_CHANNELS],
            }; MAX_AUDIO_MIXES],
        }
    }
}

/// `obs_transition_audio_mix_callback_t`, NULL allowed at the boundary.
pub type MixCallback = Option<unsafe extern "C" fn(data: *mut c_void, t: f32) -> f32>;

unsafe extern "C" {
    fn obs_source_transition_seam_set_alignment(transition: *mut c_void, value: u32);
    fn obs_source_transition_seam_get_texrender(
        transition: *const c_void,
        idx: usize,
    ) -> *mut c_void;
    fn obs_source_transition_seam_set_texrender(
        transition: *mut c_void,
        idx: usize,
        value: *mut c_void,
    );
    fn obs_source_transition_seam_get_transitioning_video(transition: *const c_void) -> bool;
    fn obs_source_transition_seam_get_transitioning_audio(transition: *const c_void) -> bool;
    fn obs_source_transition_seam_set_transitioning_audio(transition: *mut c_void, value: bool);
    fn obs_source_transition_seam_get_context_data(transition: *const c_void) -> *mut c_void;
    fn obs_source_transition_seam_get_child_audio_pending(child: *const c_void) -> bool;
    fn obs_source_transition_seam_get_child_audio_is_duplicated(child: *const c_void) -> bool;
    fn obs_source_transition_seam_get_child_audio_output_buf(child: *const c_void) -> *mut f32;
    fn obs_source_transition_seam_init_mutex_values(transition: *mut c_void);
    fn obs_source_transition_seam_mutex_init(transition: *mut c_void) -> i32;
    fn obs_source_transition_seam_tex_mutex_init(transition: *mut c_void) -> i32;
    fn obs_source_transition_seam_mutex_destroy(transition: *mut c_void);
    fn obs_source_transition_seam_tex_mutex_destroy(transition: *mut c_void);
}

/// The calls the audio render, init and free make into the rest of libobs,
/// one C function each, beyond the stop shims' [`stop::Calls`].
///
/// # Safety
/// Each is called with the pointers the C passes: live sources (a child or
/// NULL where the C allows NULL), and texrenders and a graphics context the
/// C read.
pub unsafe trait Calls: stop::Calls {
    /// `obs_source_removed(source)`.
    ///
    /// # Safety
    /// `source` is a live `obs_source_t`.
    unsafe fn removed(source: *mut c_void) -> bool;
    /// `obs_source_get_audio_timestamp(source)`.
    ///
    /// # Safety
    /// `source` is a live `obs_source_t`.
    unsafe fn audio_timestamp(source: *mut c_void) -> u64;
    /// `obs_source_get_audio_mix(source, audio)`.
    ///
    /// # Safety
    /// `source` is a live `obs_source_t`, `audio` writable.
    unsafe fn audio_mix(source: *mut c_void, audio: *mut AudioMix);
    /// `obs_source_get_ref(source)`.
    ///
    /// # Safety
    /// `source` is NULL or a live `obs_source_t`.
    unsafe fn get_ref(source: *mut c_void) -> *mut c_void;
    /// `obs->video.graphics`, through
    /// `obs_source_transition_seam_get_graphics()`.
    ///
    /// # Safety
    /// libobs is started.
    unsafe fn graphics() -> *mut c_void;
    /// `gs_texrender_create(GS_RGBA, GS_ZS_NONE)`.
    ///
    /// # Safety
    /// libobs is started.
    unsafe fn texrender_create() -> *mut c_void;
    /// `gs_texrender_destroy(texrender)`.
    ///
    /// # Safety
    /// `texrender` is NULL or one `gs_texrender_create()` made.
    unsafe fn texrender_destroy(texrender: *mut c_void);
    /// `gs_enter_context(graphics)`.
    ///
    /// # Safety
    /// `graphics` is NULL or the core's graphics context.
    unsafe fn enter_context(graphics: *mut c_void);
    /// `gs_leave_context()`.
    ///
    /// # Safety
    /// After [`Calls::enter_context`].
    unsafe fn leave_context();
}

/// [`Calls`] through libobs's own functions.
pub use stop::Libobs;

/// `GS_RGBA` (`graphics/graphics.h`), the texrender format
/// `obs_transition_init()` asks for.
pub const GS_RGBA: i32 = 3;

/// `GS_ZS_NONE` (`graphics/graphics.h`), the texrender depth format
/// `obs_transition_init()` asks for.
pub const GS_ZS_NONE: i32 = 0;

unsafe extern "C" {
    fn obs_source_removed(source: *mut c_void) -> bool;
    fn obs_source_get_audio_timestamp(source: *mut c_void) -> u64;
    fn obs_source_get_audio_mix(source: *mut c_void, audio: *mut AudioMix);
    fn obs_source_get_ref(source: *mut c_void) -> *mut c_void;
    fn obs_source_transition_seam_get_graphics() -> *mut c_void;
    fn gs_texrender_create(format: i32, zsformat: i32) -> *mut c_void;
    fn gs_texrender_destroy(texrender: *mut c_void);
    fn gs_enter_context(graphics: *mut c_void);
    fn gs_leave_context();
}

// SAFETY: each method is the one libobs function of its name, called with
// the caller's pointers.
unsafe impl Calls for Libobs {
    unsafe fn removed(source: *mut c_void) -> bool {
        // SAFETY: see the trait.
        unsafe { obs_source_removed(source) }
    }
    unsafe fn audio_timestamp(source: *mut c_void) -> u64 {
        // SAFETY: see the trait.
        unsafe { obs_source_get_audio_timestamp(source) }
    }
    unsafe fn audio_mix(source: *mut c_void, audio: *mut AudioMix) {
        // SAFETY: see the trait.
        unsafe { obs_source_get_audio_mix(source, audio) }
    }
    unsafe fn get_ref(source: *mut c_void) -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { obs_source_get_ref(source) }
    }
    unsafe fn graphics() -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { obs_source_transition_seam_get_graphics() }
    }
    unsafe fn texrender_create() -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { gs_texrender_create(GS_RGBA, GS_ZS_NONE) }
    }
    unsafe fn texrender_destroy(texrender: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { gs_texrender_destroy(texrender) }
    }
    unsafe fn enter_context(graphics: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { gs_enter_context(graphics) }
    }
    unsafe fn leave_context() {
        // SAFETY: see the trait.
        unsafe { gs_leave_context() }
    }
}

/// A pointer the C hands on, never dereferenced here: a texrender or the
/// graphics context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Opaque(*mut c_void);

fn opaque(ptr: *mut c_void) -> Option<Opaque> {
    (!ptr.is_null()).then_some(Opaque(ptr))
}

fn opaque_ptr(value: Option<Opaque>) -> *mut c_void {
    value.map_or(ptr::null_mut(), |o| o.0)
}

fn child_ptr(child: Option<ChildSource>) -> *mut c_void {
    child.map_or(ptr::null_mut(), ChildSource::as_ptr)
}

/// The caller's mix, the `struct obs_source_audio_mix *` the C was handed.
#[derive(Clone, Copy, Debug)]
pub struct MixPtr(*mut AudioMix);

/// The transition's `context.data`, handed to its mix callbacks.
#[derive(Clone, Copy, Debug)]
pub struct ContextData(*mut c_void);

/// The samples of one channel, a `float *` of a mix or of a child's block.
#[derive(Clone, Copy, Debug)]
pub struct Samples(*mut f32);

/// `mix->output[mix_idx].data[ch]`, indexed as the C indexes it: through
/// the pointer, unchecked.
///
/// # Safety
/// `mix` points at a `struct obs_source_audio_mix`, `mix_idx` is below
/// `MAX_AUDIO_MIXES` and `ch` a channel the caller asked for.
unsafe fn channel(mix: *const AudioMix, mix_idx: usize, ch: usize) -> *mut f32 {
    // SAFETY: the caller's contract.
    unsafe {
        ptr::addr_of!((*mix).output)
            .cast::<AudioOutputData>()
            .add(mix_idx)
            .cast::<*mut f32>()
            .add(ch)
            .read()
    }
}

// SAFETY (every method): `self.as_ptr()` is the live transition the C
// caller passed, each child one the seam read from its slot (live while
// the slot or the render holds a reference on it), and each call is the
// one seam accessor or libobs function that names it. `idx` is a slot, 0
// or 1. The mixes are the caller's `struct obs_source_audio_mix` (whose
// channels are its buffers of AUDIO_OUTPUT_FRAMES floats) and children's
// as `obs_source_get_audio_mix()` filled them, the samples a frame below
// AUDIO_OUTPUT_FRAMES of one of their channels, as the C indexes them.
impl<L: Calls> Transition for Seam<L> {
    type Texrender = Opaque;
    type Graphics = Opaque;
    type Data = ContextData;
    type MixCallback = MixCallback;
    type Mix = MixPtr;
    type ChildAudio = AudioMix;
    type Samples = Samples;

    fn init_mutex_values(&self) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_init_mutex_values(self.as_ptr()) }
    }
    fn mutex_init(&self) -> i32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_mutex_init(self.as_ptr()) }
    }
    fn tex_mutex_init(&self) -> i32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_tex_mutex_init(self.as_ptr()) }
    }
    fn mutex_destroy(&self) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_mutex_destroy(self.as_ptr()) }
    }
    fn tex_mutex_destroy(&self) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_tex_mutex_destroy(self.as_ptr()) }
    }
    fn set_alignment(&self, value: u32) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_alignment(self.as_ptr(), value) }
    }
    fn texrender(&self, idx: usize) -> Option<Opaque> {
        // SAFETY: see the impl.
        opaque(unsafe { obs_source_transition_seam_get_texrender(self.as_ptr(), idx) })
    }
    fn set_texrender(&self, idx: usize, value: Option<Opaque>) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_texrender(self.as_ptr(), idx, opaque_ptr(value)) }
    }
    fn transitioning_video(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_transitioning_video(self.as_ptr()) }
    }
    fn transitioning_audio(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_transitioning_audio(self.as_ptr()) }
    }
    fn set_transitioning_audio(&self, value: bool) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_transitioning_audio(self.as_ptr(), value) }
    }
    fn context_data(&self) -> ContextData {
        // SAFETY: see the impl.
        ContextData(unsafe { obs_source_transition_seam_get_context_data(self.as_ptr()) })
    }
    fn graphics(&self) -> Option<Opaque> {
        // SAFETY: see the impl; the caller's libobs is up.
        opaque(unsafe { L::graphics() })
    }
    fn texrender_create(&self) -> Option<Opaque> {
        // SAFETY: see the impl.
        opaque(unsafe { L::texrender_create() })
    }
    fn texrender_destroy(&self, texrender: Option<Opaque>) {
        // SAFETY: see the impl; NULL as the C passes it.
        unsafe { L::texrender_destroy(opaque_ptr(texrender)) }
    }
    fn enter_context(&self, graphics: Option<Opaque>) {
        // SAFETY: see the impl; NULL as the C passes it.
        unsafe { L::enter_context(opaque_ptr(graphics)) }
    }
    fn leave_context(&self) {
        // SAFETY: see the impl.
        unsafe { L::leave_context() }
    }
    fn removed(&self, child: ChildSource) -> bool {
        // SAFETY: see the impl.
        unsafe { L::removed(child.as_ptr()) }
    }
    fn child_audio_pending(&self, child: ChildSource) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_child_audio_pending(child.as_ptr()) }
    }
    fn child_audio_is_duplicated(&self, child: ChildSource) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_child_audio_is_duplicated(child.as_ptr()) }
    }
    fn audio_timestamp(&self, child: ChildSource) -> u64 {
        // SAFETY: see the impl.
        unsafe { L::audio_timestamp(child.as_ptr()) }
    }
    fn get_ref(&self, child: Option<ChildSource>) -> Option<ChildSource> {
        // SAFETY: see the impl; NULL as the C passes it.
        ChildSource::from_ptr(unsafe { L::get_ref(child_ptr(child)) })
    }
    fn audio_mix(&self, child: ChildSource) -> AudioMix {
        let mut audio = AudioMix::empty();
        // SAFETY: see the impl; `audio` is a local.
        unsafe { L::audio_mix(child.as_ptr(), &raw mut audio) };
        audio
    }
    fn child_audio_output_buf(&self, child: ChildSource) -> Samples {
        // SAFETY: see the impl.
        Samples(unsafe { obs_source_transition_seam_get_child_audio_output_buf(child.as_ptr()) })
    }
    fn call_mix(&self, mix: MixCallback, data: ContextData, t: f32) -> f32 {
        let Some(mix) = mix else {
            // The C calls through the NULL pointer; stop as hard.
            std::process::abort();
        };
        // SAFETY: the type's callback, with its own context data.
        unsafe { mix(data.0, t) }
    }
    fn output_samples(&self, audio: MixPtr, mix_idx: usize, ch: usize) -> Samples {
        // SAFETY: see the impl.
        Samples(unsafe { channel(audio.0.cast_const(), mix_idx, ch) })
    }
    fn input_samples(&self, audio: &AudioMix, mix_idx: usize, ch: usize) -> Samples {
        // SAFETY: see the impl; a child's mix, as obs_source_get_audio_mix()
        // filled it.
        Samples(unsafe { channel(ptr::from_ref(audio), mix_idx, ch) })
    }
    fn sample(&self, samples: Samples, frame: usize) -> f32 {
        // SAFETY: see the impl.
        unsafe { samples.0.add(frame).read() }
    }
    fn set_sample(&self, samples: Samples, frame: usize, value: f32) {
        // SAFETY: see the impl.
        unsafe { samples.0.add(frame).write(value) }
    }
    fn copy_audio(&self, audio: MixPtr, from: Samples) {
        // SAFETY: see the impl; the C's memcpy() of TOTAL_AUDIO_SIZE bytes
        // from the child's one block into the caller's.
        unsafe {
            let to = (*audio.0).output[0].data[0];
            ptr::copy(from.0.cast_const(), to, TOTAL_AUDIO_FLOATS);
        }
    }
}

/// `obs_transition_init()` with the libobs calls of `L`.
///
/// # Safety
/// `transition` is a transition `obs_source_t` being created, its mutexes
/// not yet made.
pub unsafe fn init<L: Calls>(transition: *mut c_void) -> bool {
    core::init(&Seam::<L>::new(transition))
}

/// `obs_transition_free()` with the libobs calls of `L`.
///
/// # Safety
/// `transition` is a transition `obs_source_t` being destroyed, after
/// [`init`] made its mutexes.
pub unsafe fn free<L: Calls>(transition: *mut c_void) {
    core::free(&Seam::<L>::new(transition));
}

/// `obs_transition_audio_render()` with the libobs calls of `L`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`; for a transition,
/// `ts_out` is writable and `audio` the caller's mix, with a buffer of
/// AUDIO_OUTPUT_FRAMES floats for each mix and channel it selects (and one
/// block of every mix and channel at `output[0].data[0]`), as the C needs.
#[allow(clippy::too_many_arguments)]
pub unsafe fn audio_render<L: Calls>(
    transition: *mut c_void,
    ts_out: *mut u64,
    audio: *mut AudioMix,
    mixers: u32,
    channels: usize,
    sample_rate: usize,
    mix_a: MixCallback,
    mix_b: MixCallback,
) -> bool {
    let seam = (!transition.is_null()).then(|| Seam::<L>::new(transition));
    let request = Request {
        audio: MixPtr(audio),
        mixers,
        channels,
        sample_rate,
        mix_a,
        mix_b,
    };
    let Some(ts) = core::audio_render(seam.as_ref(), request) else {
        return false;
    };
    // SAFETY: the caller's contract.
    unsafe { ts_out.write(ts) };
    ts != 0
}

/// `bool obs_transition_init(obs_source_t *transition)`.
///
/// # Safety
/// As for [`init`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_init(transition: *mut c_void) -> bool {
    // SAFETY: the caller's contract.
    unsafe { init::<Libobs>(transition) }
}

/// `void obs_transition_free(obs_source_t *transition)`.
///
/// # Safety
/// As for [`free`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_free(transition: *mut c_void) {
    // SAFETY: the caller's contract.
    unsafe { free::<Libobs>(transition) }
}

/// `bool obs_transition_audio_render(obs_source_t *transition, uint64_t
/// *ts_out, struct obs_source_audio_mix *audio, uint32_t mixers, size_t
/// channels, size_t sample_rate, obs_transition_audio_mix_callback_t mix_a,
/// obs_transition_audio_mix_callback_t mix_b)`.
///
/// # Safety
/// As for [`audio_render`].
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn obs_transition_audio_render(
    transition: *mut c_void,
    ts_out: *mut u64,
    audio: *mut AudioMix,
    mixers: u32,
    channels: usize,
    sample_rate: usize,
    mix_a: MixCallback,
    mix_b: MixCallback,
) -> bool {
    // SAFETY: the caller's contract.
    unsafe {
        audio_render::<Libobs>(
            transition,
            ts_out,
            audio,
            mixers,
            channels,
            sample_rate,
            mix_a,
            mix_b,
        )
    }
}
