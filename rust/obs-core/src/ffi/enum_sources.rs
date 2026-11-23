//! `obs_enum_sources()`, `obs_enum_all_sources()`, `obs_enum_canvases()` and
//! `obs_canvas_enum_scenes()` (#5495) over C-owned state: the core's source
//! table, its named canvases and a canvas's sources, their mutexes and
//! links, `obs_source_get_ref`/`obs_source_release` and the source fields
//! read, behind the private adapters of `libobs/obs-enum-sources-fields.c.h`.
//! The walks are the safe core's ([`crate::enum_sources`]); the caller's
//! callback is called here, with its `param` and the entry.
use core::ffi::c_void;
use core::ptr::NonNull;

use crate::enum_sources::{
    Referenced, Scenes, Sources, Table, canvas_enum_scenes, enum_canvases, enum_sources, enum_uuid,
};

/// `obs_source_t`, opaque here.
pub type Source = c_void;
/// `obs_canvas_t`, opaque here.
pub type Canvas = c_void;

/// The caller's callback: `bool (*)(void *param, obs_source_t *source)` or
/// `bool (*)(void *param, obs_canvas_t *canvas)`.
pub type EnumProc = Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool>;

unsafe extern "C" {
    fn obs_enum_sources_lock();
    fn obs_enum_sources_unlock();
    fn obs_enum_sources_first() -> *mut Source;
    fn obs_enum_sources_next(source: *const Source) -> *mut Source;
    fn obs_enum_sources_get_ref(source: *mut Source) -> *mut Source;
    fn obs_enum_sources_release(source: *mut Source);
    fn obs_enum_sources_private(source: *const Source) -> bool;
    fn obs_enum_sources_input(source: *const Source) -> bool;
    fn obs_enum_sources_group(source: *const Source) -> bool;
    fn obs_enum_sources_scene(source: *const Source) -> bool;
    fn obs_enum_sources_canvas_lock(canvas: *mut Canvas);
    fn obs_enum_sources_canvas_unlock(canvas: *mut Canvas);
    fn obs_enum_sources_canvas_first(canvas: *const Canvas) -> *mut Source;
    fn obs_enum_sources_canvas_next(source: *const Source) -> *mut Source;
    fn obs_enum_sources_canvases_lock();
    fn obs_enum_sources_canvases_unlock();
    fn obs_enum_sources_canvases_first() -> *mut Canvas;
    fn obs_enum_sources_canvases_next(canvas: *const Canvas) -> *mut Canvas;
}

/// The caller's callback and `param`.
struct Callback {
    enum_proc: unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool,
    param: *mut c_void,
}

impl Callback {
    fn call(&self, entry: NonNull<c_void>) -> bool {
        // SAFETY: the caller's callback and param, as passed; the entry is
        // listed (and, for the referenced walks, referenced) for the call.
        // The callback must not unwind across the C boundary.
        unsafe { (self.enum_proc)(self.param, entry.as_ptr()) }
    }
}

/// The core's source table (`obs_enum_sources()` and
/// `obs_enum_all_sources()`).
struct CoreSources(Callback);

impl Table for CoreSources {
    type Entry = NonNull<Source>;
    fn lock(&mut self) {
        // SAFETY: the initialized core's re-entrant sources mutex.
        unsafe { obs_enum_sources_lock() }
    }
    fn unlock(&mut self) {
        // SAFETY: paired with this walk's own `lock`.
        unsafe { obs_enum_sources_unlock() }
    }
    fn first(&mut self) -> Option<NonNull<Source>> {
        // SAFETY: the table head, read under the sources mutex.
        NonNull::new(unsafe { obs_enum_sources_first() })
    }
    fn next(&mut self, entry: NonNull<Source>) -> Option<NonNull<Source>> {
        // SAFETY: a listed source, live while the sources mutex is held.
        NonNull::new(unsafe { obs_enum_sources_next(entry.as_ptr()) })
    }
    fn visit(&mut self, entry: NonNull<Source>) -> bool {
        self.0.call(entry)
    }
}

impl Referenced for CoreSources {
    fn get_ref(&mut self, entry: NonNull<Source>) -> Option<NonNull<Source>> {
        // SAFETY: a listed source under the sources mutex; one with no
        // reference left answers NULL.
        NonNull::new(unsafe { obs_enum_sources_get_ref(entry.as_ptr()) })
    }
    fn release(&mut self, entry: NonNull<Source>) {
        // SAFETY: a reference this walk took with `get_ref`, released once.
        unsafe { obs_enum_sources_release(entry.as_ptr()) }
    }
}

impl Sources for CoreSources {
    fn private(&mut self, source: NonNull<Source>) -> bool {
        // SAFETY: a referenced source.
        unsafe { obs_enum_sources_private(source.as_ptr()) }
    }
    fn input(&mut self, source: NonNull<Source>) -> bool {
        // SAFETY: a referenced source.
        unsafe { obs_enum_sources_input(source.as_ptr()) }
    }
    fn group(&mut self, source: NonNull<Source>) -> bool {
        // SAFETY: a referenced source; its id is a C string.
        unsafe { obs_enum_sources_group(source.as_ptr()) }
    }
}

/// One canvas's sources (`obs_canvas_enum_scenes()`).
struct CanvasSources {
    canvas: *mut Canvas,
    callback: Callback,
}

impl Table for CanvasSources {
    type Entry = NonNull<Source>;
    fn lock(&mut self) {
        // SAFETY: the caller's live canvas; its sources mutex is re-entrant.
        unsafe { obs_enum_sources_canvas_lock(self.canvas) }
    }
    fn unlock(&mut self) {
        // SAFETY: paired with this walk's own `lock`.
        unsafe { obs_enum_sources_canvas_unlock(self.canvas) }
    }
    fn first(&mut self) -> Option<NonNull<Source>> {
        // SAFETY: the canvas's head, read under its sources mutex.
        NonNull::new(unsafe { obs_enum_sources_canvas_first(self.canvas) })
    }
    fn next(&mut self, entry: NonNull<Source>) -> Option<NonNull<Source>> {
        // SAFETY: a listed source, live while the canvas's mutex is held.
        NonNull::new(unsafe { obs_enum_sources_canvas_next(entry.as_ptr()) })
    }
    fn visit(&mut self, entry: NonNull<Source>) -> bool {
        self.callback.call(entry)
    }
}

impl Referenced for CanvasSources {
    fn get_ref(&mut self, entry: NonNull<Source>) -> Option<NonNull<Source>> {
        // SAFETY: a listed source under the canvas's mutex.
        NonNull::new(unsafe { obs_enum_sources_get_ref(entry.as_ptr()) })
    }
    fn release(&mut self, entry: NonNull<Source>) {
        // SAFETY: a reference this walk took with `get_ref`, released once.
        unsafe { obs_enum_sources_release(entry.as_ptr()) }
    }
}

impl Scenes for CanvasSources {
    fn scene(&mut self, source: NonNull<Source>) -> bool {
        // SAFETY: a listed source, live under the canvas's mutex.
        unsafe { obs_enum_sources_scene(source.as_ptr()) }
    }
}

/// The core's named canvases (`obs_enum_canvases()`).
struct NamedCanvases(Callback);

impl Table for NamedCanvases {
    type Entry = NonNull<Canvas>;
    fn lock(&mut self) {
        // SAFETY: the initialized core's canvases mutex.
        unsafe { obs_enum_sources_canvases_lock() }
    }
    fn unlock(&mut self) {
        // SAFETY: paired with this walk's own `lock`.
        unsafe { obs_enum_sources_canvases_unlock() }
    }
    fn first(&mut self) -> Option<NonNull<Canvas>> {
        // SAFETY: the initialized core's head of the named canvases, read
        // before the lock as the original reads it.
        NonNull::new(unsafe { obs_enum_sources_canvases_first() })
    }
    fn next(&mut self, entry: NonNull<Canvas>) -> Option<NonNull<Canvas>> {
        // SAFETY: a named canvas, live while the canvases mutex is held.
        NonNull::new(unsafe { obs_enum_sources_canvases_next(entry.as_ptr()) })
    }
    fn visit(&mut self, entry: NonNull<Canvas>) -> bool {
        self.0.call(entry)
    }
}

/// The caller's callback; the C asserted one (or only ever called it), so
/// without one there is nothing to walk.
fn callback(enum_proc: EnumProc, param: *mut c_void) -> Option<Callback> {
    enum_proc.map(|enum_proc| Callback { enum_proc, param })
}

/// Calls `enum_proc(param, source)` for every public input and group, each
/// referenced for the call, until it answers false.
///
/// # Safety
/// As for the C function: the core is initialized and `enum_proc` is a
/// valid callback for `param` that does not unwind.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_enum_sources(enum_proc: EnumProc, param: *mut c_void) {
    if let Some(callback) = callback(enum_proc, param) {
        enum_sources(&mut CoreSources(callback));
    }
}

/// Calls `enum_proc(param, source)` for every source of the core, private
/// ones and ones with no reference left included, without a reference,
/// until it answers false.
///
/// # Safety
/// As for [`obs_enum_sources`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_enum_all_sources(enum_proc: EnumProc, param: *mut c_void) {
    if let Some(callback) = callback(enum_proc, param) {
        enum_uuid(&mut CoreSources(callback));
    }
}

/// Calls `enum_proc(param, canvas)` for every named canvas, without a
/// reference, until it answers false.
///
/// # Safety
/// As for [`obs_enum_sources`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_enum_canvases(enum_proc: EnumProc, param: *mut c_void) {
    if let Some(callback) = callback(enum_proc, param) {
        enum_canvases(&mut NamedCanvases(callback));
    }
}

/// Calls `enum_proc(param, scene)` for every scene of `canvas`, each
/// referenced for the call, until it answers false.
///
/// # Safety
/// As for the C function: `canvas` is a live canvas of the initialized
/// core and `enum_proc` is a valid callback for `param` that does not
/// unwind.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_canvas_enum_scenes(
    canvas: *mut Canvas,
    enum_proc: EnumProc,
    param: *mut c_void,
) {
    if let Some(callback) = callback(enum_proc, param) {
        canvas_enum_scenes(&mut CanvasSources { canvas, callback });
    }
}
