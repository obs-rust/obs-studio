//! Safe wrappers over the libobs C API for the in-process backend (#3283).
//!
//! This is the only module of the crate that calls into libobs. Each wrapper
//! owns exactly the reference libobs hands out and releases it on drop, turns
//! C strings and null returns into `String` and `Result`, and keeps the
//! `unsafe` blocks to a single call each with its invariant written down.
//!
//! libobs must be started (`obs_startup`) before any wrapper is used and must
//! outlive every value returned here; the host owns that lifecycle.

use std::ffi::{CStr, CString, c_char, c_void};
use std::ptr::NonNull;

use obs_sys::ffi as sys;
use serde_json::Value;

/// A NUL-free C string for `value`, or an error naming `what`.
fn c_string(value: &str, what: &str) -> Result<CString, String> {
    CString::new(value).map_err(|_| format!("{what} must not contain a NUL byte"))
}

/// Copies a C string owned by libobs.
///
/// # Safety
///
/// `ptr` is null or a NUL-terminated string that stays valid for the call.
unsafe fn text(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        // SAFETY: non-null and NUL-terminated per the function contract.
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

/// An owned `obs_data_t` built from JSON.
struct Data(NonNull<sys::obs_data_t>);

impl Data {
    fn from_json(value: &Value) -> Result<Self, String> {
        let json = c_string(&value.to_string(), "settings")?;
        // SAFETY: `json` is NUL-terminated; libobs returns an owned object or null.
        let data = unsafe { sys::obs_data_create_from_json(json.as_ptr()) };
        NonNull::new(data)
            .map(Self)
            .ok_or_else(|| "settings are not a valid JSON object".to_owned())
    }
}

impl Drop for Data {
    fn drop(&mut self) {
        // SAFETY: owns one reference taken by `obs_data_create_from_json`.
        unsafe { sys::obs_data_release(self.0.as_ptr()) };
    }
}

/// An owned reference to an `obs_source_t` (a scene is a source too).
pub struct Source(NonNull<sys::obs_source_t>);

impl Source {
    /// The source called `name`, if it exists.
    pub fn by_name(name: &str) -> Option<Self> {
        let name = CString::new(name).ok()?;
        // SAFETY: `name` is NUL-terminated; the result is an owned reference or null.
        NonNull::new(unsafe { sys::obs_get_source_by_name(name.as_ptr()) }).map(Self)
    }

    /// Creates a public source of `kind` called `name` and returns the
    /// reference that keeps it alive: libobs destroys a source whose last
    /// reference is released.
    pub fn create(kind: &str, name: &str, settings: &Value) -> Result<Self, String> {
        let kind_c = c_string(kind, "kind")?;
        let name_c = c_string(name, "name")?;
        // libobs keeps a source of a kind no module registered as a placeholder
        // for the missing plugin (and logs an error) instead of returning null.
        // SAFETY: `kind_c` is NUL-terminated; the result is borrowed or null.
        if unsafe { sys::obs_source_get_display_name(kind_c.as_ptr()) }.is_null() {
            return Err(format!("cannot create a source of unknown kind '{kind}'"));
        }
        let data = Data::from_json(settings)?;
        // SAFETY: both strings are NUL-terminated and `data` is live; a null
        // hotkey store is allowed; the result is an owned reference or null.
        let source = unsafe {
            sys::obs_source_create(
                kind_c.as_ptr(),
                name_c.as_ptr(),
                data.0.as_ptr(),
                std::ptr::null_mut(),
            )
        };
        NonNull::new(source)
            .map(Self)
            .ok_or_else(|| format!("cannot create a source of unknown kind '{kind}'"))
    }

    /// Creates a scene called `name`; see [`Source::create`] for ownership.
    pub fn create_scene(name: &str) -> Result<Self, String> {
        let name_c = c_string(name, "name")?;
        // SAFETY: `name_c` is NUL-terminated; the result is an owned scene or null.
        let scene = NonNull::new(unsafe { sys::obs_scene_create(name_c.as_ptr()) })
            .ok_or_else(|| format!("cannot create scene '{name}'"))?;
        // SAFETY: `scene` is live; its source is borrowed, so take a source
        // reference of our own and give back the scene reference.
        unsafe {
            let source = sys::obs_source_get_ref(sys::obs_scene_get_source(scene.as_ptr()));
            sys::obs_scene_release(scene.as_ptr());
            NonNull::new(source)
                .map(Self)
                .ok_or_else(|| format!("scene '{name}' vanished after creation"))
        }
    }

    /// The source name.
    pub fn name(&self) -> String {
        // SAFETY: the source is live; the string is copied before the call returns.
        unsafe { text(sys::obs_source_get_name(self.0.as_ptr())) }
    }

    /// Whether this source is a scene.
    pub fn is_scene(&self) -> bool {
        // SAFETY: the source is live.
        unsafe { sys::obs_source_get_type(self.0.as_ptr()) == sys::OBS_SOURCE_TYPE_SCENE }
    }

    /// Merges `settings` into the source's settings.
    pub fn update(&self, settings: &Value) -> Result<(), String> {
        let data = Data::from_json(settings)?;
        // SAFETY: both objects are live; libobs copies what it needs.
        unsafe { sys::obs_source_update(self.0.as_ptr(), data.0.as_ptr()) };
        Ok(())
    }

    /// Removes the source from libobs' public lists; it dies with its last reference.
    pub fn remove(&self) {
        // SAFETY: the source is live.
        unsafe { sys::obs_source_remove(self.0.as_ptr()) };
    }

    /// The source's size in pixels.
    pub fn size(&self) -> (u32, u32) {
        // SAFETY: the source is live.
        unsafe {
            (
                sys::obs_source_get_width(self.0.as_ptr()),
                sys::obs_source_get_height(self.0.as_ptr()),
            )
        }
    }

    /// Runs `f` on the scene item for `child` of this scene.
    ///
    /// The item is borrowed from the scene, so it is only valid inside `f`.
    pub fn with_item<T>(&self, child: &str, f: impl FnOnce(&Item<'_>) -> T) -> Result<T, String> {
        let child_c = c_string(child, "source name")?;
        // SAFETY: the source is live; `obs_scene_from_source` returns null for
        // a non-scene and `obs_scene_find_source` returns a borrowed item or null.
        let item = unsafe {
            let scene = sys::obs_scene_from_source(self.0.as_ptr());
            if scene.is_null() {
                return Err(format!("'{}' is not a scene", self.name()));
            }
            sys::obs_scene_find_source(scene, child_c.as_ptr())
        };
        let item = NonNull::new(item)
            .ok_or_else(|| format!("'{child}' is not in scene '{}'", self.name()))?;
        Ok(f(&Item {
            ptr: item,
            _scene: std::marker::PhantomData,
        }))
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        // SAFETY: owns one reference.
        unsafe { sys::obs_source_release(self.0.as_ptr()) };
    }
}

/// A scene item borrowed from its scene inside [`Source::with_item`].
pub struct Item<'a> {
    ptr: NonNull<sys::obs_sceneitem_t>,
    _scene: std::marker::PhantomData<&'a Source>,
}

impl Item<'_> {
    /// Shows or hides the item.
    pub fn set_visible(&self, visible: bool) {
        // SAFETY: the item is live for the borrow.
        unsafe { sys::obs_sceneitem_set_visible(self.ptr.as_ptr(), visible) };
    }

    /// Position, scale and rotation in degrees.
    pub fn transform(&self) -> ((f32, f32), (f32, f32), f32) {
        let (mut pos, mut scale) = (sys::vec2::default(), sys::vec2::default());
        // SAFETY: the item is live; the out-pointers point at initialized locals.
        let rot = unsafe {
            sys::obs_sceneitem_get_pos(self.ptr.as_ptr(), &raw mut pos);
            sys::obs_sceneitem_get_scale(self.ptr.as_ptr(), &raw mut scale);
            sys::obs_sceneitem_get_rot(self.ptr.as_ptr())
        };
        ((pos.x, pos.y), (scale.x, scale.y), rot)
    }

    /// Sets position, scale and rotation in degrees.
    pub fn set_transform(&self, pos: (f32, f32), scale: (f32, f32), rot: f32) {
        let pos = sys::vec2 { x: pos.0, y: pos.1 };
        let scale = sys::vec2 {
            x: scale.0,
            y: scale.1,
        };
        // SAFETY: the item is live; libobs copies the vectors.
        unsafe {
            sys::obs_sceneitem_set_pos(self.ptr.as_ptr(), &raw const pos);
            sys::obs_sceneitem_set_scale(self.ptr.as_ptr(), &raw const scale);
            sys::obs_sceneitem_set_rot(self.ptr.as_ptr(), rot);
        }
    }
}

/// Collects `(name, kind id)` of each source an enumerator visits.
unsafe extern "C" fn collect(param: *mut c_void, source: *mut sys::obs_source_t) -> bool {
    // SAFETY: `param` is the `Vec` passed to the enumerator below and `source`
    // is valid for the callback (libobs does not hand out a reference).
    unsafe {
        let list = &mut *param.cast::<Vec<(String, String)>>();
        list.push((
            text(sys::obs_source_get_name(source)),
            text(sys::obs_source_get_id(source)),
        ));
    }
    true
}

/// `(name, kind id)` of every public input source.
pub fn inputs() -> Vec<(String, String)> {
    let mut list = Vec::new();
    // SAFETY: `collect` only touches the `Vec` passed as `param`, which outlives the call.
    unsafe { sys::obs_enum_sources(collect, (&raw mut list).cast()) };
    list
}

/// `(name, kind id)` of every public scene.
pub fn scenes() -> Vec<(String, String)> {
    let mut list = Vec::new();
    // SAFETY: as in [`inputs`].
    unsafe { sys::obs_enum_scenes(collect, (&raw mut list).cast()) };
    list
}

/// The source on program output channel 0.
pub fn program() -> Option<Source> {
    // SAFETY: returns an owned reference or null.
    NonNull::new(unsafe { sys::obs_get_output_source(0) }).map(Source)
}

/// Sets (or with `None` clears) the source on program output channel 0.
pub fn set_program(source: Option<&Source>) {
    let ptr = source.map_or(std::ptr::null_mut(), |s| s.0.as_ptr());
    // SAFETY: null or a live source; libobs takes its own reference.
    unsafe { sys::obs_set_output_source(0, ptr) };
}

/// An owned reference to an `obs_output_t`.
pub struct Output(NonNull<sys::obs_output_t>);

impl Output {
    /// The output called `name`, if it exists.
    pub fn by_name(name: &str) -> Option<Self> {
        let name = CString::new(name).ok()?;
        // SAFETY: `name` is NUL-terminated; the result is an owned reference or null.
        NonNull::new(unsafe { sys::obs_get_output_by_name(name.as_ptr()) }).map(Self)
    }

    /// Creates a public output of `kind` called `name`; libobs destroys it when
    /// the last reference is released, so the caller keeps this one.
    pub fn create(kind: &str, name: &str, settings: &Value) -> Result<Self, String> {
        let kind_c = c_string(kind, "kind")?;
        let name_c = c_string(name, "name")?;
        let data = Data::from_json(settings)?;
        // SAFETY: both strings are NUL-terminated and `data` is live; a null
        // hotkey store is allowed; the result is an owned reference or null.
        let output = unsafe {
            sys::obs_output_create(
                kind_c.as_ptr(),
                name_c.as_ptr(),
                data.0.as_ptr(),
                std::ptr::null_mut(),
            )
        };
        NonNull::new(output)
            .map(Self)
            .ok_or_else(|| format!("no output kind '{kind}' (is its plugin loaded?)"))
    }

    /// Merges `settings` into the output's settings.
    pub fn update(&self, settings: &Value) -> Result<(), String> {
        let data = Data::from_json(settings)?;
        // SAFETY: both objects are live; libobs copies what it needs.
        unsafe { sys::obs_output_update(self.0.as_ptr(), data.0.as_ptr()) };
        Ok(())
    }

    /// Whether the output is running.
    pub fn active(&self) -> bool {
        // SAFETY: the output is live.
        unsafe { sys::obs_output_active(self.0.as_ptr()) }
    }

    /// Starts the output, with libobs' last error on failure.
    pub fn start(&self) -> Result<(), String> {
        // SAFETY: the output is live.
        unsafe {
            if sys::obs_output_start(self.0.as_ptr()) {
                return Ok(());
            }
            let reason = text(sys::obs_output_get_last_error(self.0.as_ptr()));
            Err(if reason.is_empty() {
                "the output failed to start".to_owned()
            } else {
                format!("the output failed to start: {reason}")
            })
        }
    }

    /// Asks the output to stop; it finishes asynchronously.
    pub fn stop(&self) {
        // SAFETY: the output is live.
        unsafe { sys::obs_output_stop(self.0.as_ptr()) };
    }

    /// Bytes written or sent so far.
    pub fn total_bytes(&self) -> u64 {
        // SAFETY: the output is live.
        unsafe { sys::obs_output_get_total_bytes(self.0.as_ptr()) }
    }

    /// Frames output so far.
    pub fn total_frames(&self) -> u64 {
        // SAFETY: the output is live.
        let frames = unsafe { sys::obs_output_get_total_frames(self.0.as_ptr()) };
        u64::try_from(frames).unwrap_or(0)
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        // SAFETY: owns one reference.
        unsafe { sys::obs_output_release(self.0.as_ptr()) };
    }
}

/// Whether the video subsystem (and so a graphics context) is initialized.
pub fn has_video() -> bool {
    // SAFETY: returns a borrowed pointer or null.
    !unsafe { sys::obs_get_video() }.is_null()
}

/// The configured video frame rate, if the video subsystem is up.
pub fn video_frame_rate() -> Option<f64> {
    // SAFETY: the pointer is checked for null and only read during the call.
    unsafe {
        let video = sys::obs_get_video();
        (!video.is_null()).then(|| sys::video_output_get_frame_rate(video))
    }
}

/// Frames per second the video thread is producing.
pub fn active_fps() -> f64 {
    // SAFETY: plain query.
    unsafe { sys::obs_get_active_fps() }
}

/// Frames lagged by the render thread plus frames skipped by the encoder queue.
pub fn dropped_frames() -> u64 {
    // SAFETY: plain queries; the video pointer is checked for null.
    unsafe {
        let video = sys::obs_get_video();
        let skipped = if video.is_null() {
            0
        } else {
            sys::video_output_get_skipped_frames(video)
        };
        u64::from(sys::obs_get_lagged_frames()) + u64::from(skipped)
    }
}

/// Process CPU usage sampler (`os_cpu_usage_info_t`).
pub struct CpuUsage(NonNull<sys::os_cpu_usage_info_t>);

impl CpuUsage {
    /// Starts sampling; `None` when the platform cannot.
    pub fn start() -> Option<Self> {
        // SAFETY: returns an owned sampler or null.
        NonNull::new(unsafe { sys::os_cpu_usage_info_start() }).map(Self)
    }

    /// Percent CPU used since the previous query (or since `start`).
    pub fn query(&self) -> f64 {
        // SAFETY: the sampler is live.
        unsafe { sys::os_cpu_usage_info_query(self.0.as_ptr()) }
    }
}

impl Drop for CpuUsage {
    fn drop(&mut self) {
        // SAFETY: owns the sampler.
        unsafe { sys::os_cpu_usage_info_destroy(self.0.as_ptr()) };
    }
}

/// Holds libobs' graphics context for its lifetime.
struct GraphicsContext;

impl GraphicsContext {
    fn enter() -> Self {
        // SAFETY: balanced by `leave` in `Drop`; requires an initialized video subsystem.
        unsafe { sys::obs_enter_graphics() };
        Self
    }
}

impl Drop for GraphicsContext {
    fn drop(&mut self) {
        // SAFETY: matches `enter`.
        unsafe { sys::obs_leave_graphics() };
    }
}

/// Renders `source` (of size `from`) into a `to`-sized RGBA image, top row first.
///
/// Draws through a texrender and reads it back through a stage surface, the way
/// the frontend takes screenshots. The video subsystem must be initialized
/// ([`has_video`]).
pub fn render_rgba(source: &Source, from: (u32, u32), to: (u32, u32)) -> Result<Vec<u8>, String> {
    #[allow(clippy::cast_precision_loss, reason = "pixel sizes are far below 2^24")]
    let (from_w, from_h) = (from.0 as f32, from.1 as f32);
    let (width, height) = to;
    let row = usize::try_from(width).map_err(|e| e.to_string())? * 4;
    let rows = usize::try_from(height).map_err(|e| e.to_string())?;

    let _graphics = GraphicsContext::enter();
    // SAFETY: the graphics context is held for the whole block. The texrender
    // and stage surface are destroyed on every path below; the stage surface
    // mapping is read within `linesize * rows` bytes and unmapped before the
    // surface is destroyed.
    unsafe {
        let texrender = sys::gs_texrender_create(sys::GS_RGBA, sys::GS_ZS_NONE);
        if texrender.is_null() {
            return Err("cannot create a render target".to_owned());
        }
        let stage = sys::gs_stagesurface_create(width, height, sys::GS_RGBA);
        if stage.is_null() {
            sys::gs_texrender_destroy(texrender);
            return Err("cannot create a staging surface".to_owned());
        }
        let result = (|| {
            if !sys::gs_texrender_begin(texrender, width, height) {
                return Err("cannot begin rendering".to_owned());
            }
            let clear = sys::vec4::default();
            sys::gs_clear(sys::GS_CLEAR_COLOR, &raw const clear, 0.0, 0);
            sys::gs_ortho(0.0, from_w, 0.0, from_h, -100.0, 100.0);
            sys::gs_blend_state_push();
            sys::gs_blend_function(sys::GS_BLEND_ONE, sys::GS_BLEND_ZERO);
            sys::obs_source_inc_showing(source.0.as_ptr());
            sys::obs_source_video_render(source.0.as_ptr());
            sys::obs_source_dec_showing(source.0.as_ptr());
            sys::gs_blend_state_pop();
            sys::gs_texrender_end(texrender);

            sys::gs_stage_texture(stage, sys::gs_texrender_get_texture(texrender));
            let (mut data, mut linesize) = (std::ptr::null_mut::<u8>(), 0_u32);
            if !sys::gs_stagesurface_map(stage, &raw mut data, &raw mut linesize) {
                return Err("cannot read back the rendered image".to_owned());
            }
            let stride = linesize as usize;
            let mut rgba = Vec::with_capacity(row * rows);
            for y in 0..rows {
                rgba.extend_from_slice(std::slice::from_raw_parts(data.add(y * stride), row));
            }
            sys::gs_stagesurface_unmap(stage);
            Ok(rgba)
        })();
        sys::gs_stagesurface_destroy(stage);
        sys::gs_texrender_destroy(texrender);
        result
    }
}

mod stdio;
pub use stdio::claim_stdout;

mod setup;
pub use setup::{Pipeline, configure};

/// A started libobs; dropping it shuts libobs down.
///
/// Every other value from this module must be dropped first.
pub struct Libobs(());

impl Libobs {
    /// Starts libobs with the `en-US` locale and no module search paths.
    ///
    /// # Errors
    ///
    /// Returns an error when libobs refuses to start.
    pub fn start() -> Result<Self, String> {
        // SAFETY: the locale is a NUL-terminated static string; null optional
        // paths and name store are supported by `obs_startup`.
        let started =
            unsafe { sys::obs_startup(c"en-US".as_ptr(), std::ptr::null(), std::ptr::null_mut()) };
        if started {
            Ok(Self(()))
        } else {
            Err("obs_startup failed".to_owned())
        }
    }
}

impl Drop for Libobs {
    fn drop(&mut self) {
        // SAFETY: libobs was started by `start`, and the host dropped every
        // handle before this value.
        unsafe {
            while sys::obs_wait_for_destroy_queue() {}
            sys::obs_shutdown();
        }
    }
}
