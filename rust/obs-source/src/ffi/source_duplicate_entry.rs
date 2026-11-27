//! `obs_source_duplicate()`; the field reads and writes, the scene and group
//! lookups, the scene copy, the creates, the settings calls and the filter
//! copies stay behind the adapters of
//! `obs-source-duplicate-entry-fields.c.h`.
use crate::source_duplicate_entry::{Source, duplicate};
use core::ffi::{CStr, c_char, c_int, c_void};
use core::ptr::{self, NonNull};

unsafe extern "C" {
    fn obs_source_duplicate_entry_type(source: *const c_void) -> c_int;
    fn obs_source_duplicate_entry_scene(source: *mut c_void) -> *mut c_void;
    fn obs_source_duplicate_entry_group(source: *mut c_void) -> *mut c_void;
    fn obs_source_duplicate_entry_get_ref(source: *mut c_void) -> *mut c_void;
    fn obs_source_duplicate_entry_scene_duplicate(
        scene: *mut c_void,
        new_name: *const c_char,
        dup_type: c_int,
    ) -> *mut c_void;
    fn obs_source_duplicate_entry_scene_source(scene: *mut c_void) -> *mut c_void;
    fn obs_source_duplicate_entry_output_flags(source: *const c_void) -> u32;
    fn obs_source_duplicate_entry_settings_create() -> *mut c_void;
    fn obs_source_duplicate_entry_settings_apply(settings: *mut c_void, source: *const c_void);
    fn obs_source_duplicate_entry_create(
        source: *const c_void,
        new_name: *const c_char,
        settings: *mut c_void,
        create_private: bool,
    ) -> *mut c_void;
    fn obs_source_duplicate_entry_settings_release(settings: *mut c_void);
    fn obs_source_duplicate_entry_audio_mixers(source: *const c_void) -> u32;
    fn obs_source_duplicate_entry_set_audio_mixers(dst: *mut c_void, value: u32);
    fn obs_source_duplicate_entry_sync_offset(source: *const c_void) -> i64;
    fn obs_source_duplicate_entry_set_sync_offset(dst: *mut c_void, value: i64);
    fn obs_source_duplicate_entry_user_volume(source: *const c_void) -> f32;
    fn obs_source_duplicate_entry_set_user_volume(dst: *mut c_void, value: f32);
    fn obs_source_duplicate_entry_user_muted(source: *const c_void) -> bool;
    fn obs_source_duplicate_entry_set_user_muted(dst: *mut c_void, value: bool);
    fn obs_source_duplicate_entry_volume(source: *const c_void) -> f32;
    fn obs_source_duplicate_entry_set_volume(dst: *mut c_void, value: f32);
    fn obs_source_duplicate_entry_muted(source: *const c_void) -> bool;
    fn obs_source_duplicate_entry_set_muted(dst: *mut c_void, value: bool);
    fn obs_source_duplicate_entry_flags(source: *const c_void) -> u32;
    fn obs_source_duplicate_entry_set_flags(dst: *mut c_void, value: u32);
    fn obs_source_duplicate_entry_private_settings_apply(dst: *mut c_void, source: *const c_void);
    fn obs_source_duplicate_entry_filters(dst: *mut c_void, source: *mut c_void, private: bool);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// Constructed only for a live, non-NULL source; the new name is passed on
/// as given (NULL included).
struct Live {
    source: NonNull<c_void>,
    new_name: *const c_char,
}

impl Source for Live {
    type Ref = NonNull<c_void>;
    type Scene = NonNull<c_void>;
    /// `obs_data_create()` never returns NULL; passed on as it came.
    type Settings = *mut c_void;
    fn source_type(&self) -> i32 {
        // SAFETY: a live source; one read.
        unsafe { obs_source_duplicate_entry_type(self.source.as_ptr()) }
    }
    fn scene(&self) -> Option<NonNull<c_void>> {
        // SAFETY: a live source; the lookup reads it.
        NonNull::new(unsafe { obs_source_duplicate_entry_scene(self.source.as_ptr()) })
    }
    fn group(&self) -> Option<NonNull<c_void>> {
        // SAFETY: a live source; the lookup reads it.
        NonNull::new(unsafe { obs_source_duplicate_entry_group(self.source.as_ptr()) })
    }
    fn get_ref(&self) -> Option<NonNull<c_void>> {
        // SAFETY: a live source; NULL once its last reference is gone.
        NonNull::new(unsafe { obs_source_duplicate_entry_get_ref(self.source.as_ptr()) })
    }
    fn scene_duplicate(&self, scene: NonNull<c_void>, dup_type: i32) -> Option<NonNull<c_void>> {
        // SAFETY: the scene the lookup returned for the live source; the
        // name is the caller's, as the original passes it.
        NonNull::new(unsafe {
            obs_source_duplicate_entry_scene_duplicate(scene.as_ptr(), self.new_name, dup_type)
        })
    }
    fn scene_source(&self, scene: Option<NonNull<c_void>>) -> Option<NonNull<c_void>> {
        let scene = scene.map_or(ptr::null_mut(), NonNull::as_ptr);
        // SAFETY: the copy obs_scene_duplicate() returned, or NULL, which
        // obs_scene_get_source() answers with NULL as for the original.
        NonNull::new(unsafe { obs_source_duplicate_entry_scene_source(scene) })
    }
    fn output_flags(&self) -> u32 {
        // SAFETY: a live source; one read.
        unsafe { obs_source_duplicate_entry_output_flags(self.source.as_ptr()) }
    }
    fn settings_create(&self) -> *mut c_void {
        // SAFETY: no arguments.
        unsafe { obs_source_duplicate_entry_settings_create() }
    }
    fn settings_apply(&self, settings: *mut c_void) {
        // SAFETY: the new settings object and the live source.
        unsafe { obs_source_duplicate_entry_settings_apply(settings, self.source.as_ptr()) }
    }
    fn create(&self, settings: *mut c_void, private: bool) -> Option<NonNull<c_void>> {
        // SAFETY: the live source (its id), the caller's name and the new
        // settings object.
        NonNull::new(unsafe {
            obs_source_duplicate_entry_create(
                self.source.as_ptr(),
                self.new_name,
                settings,
                private,
            )
        })
    }
    fn settings_release(&self, settings: *mut c_void) {
        // SAFETY: the reference settings_create() returned, released once.
        unsafe { obs_source_duplicate_entry_settings_release(settings) }
    }
    fn audio_mixers(&self) -> u32 {
        // SAFETY: a live source; one read.
        unsafe { obs_source_duplicate_entry_audio_mixers(self.source.as_ptr()) }
    }
    fn set_audio_mixers(&self, s: NonNull<c_void>, value: u32) {
        // SAFETY: the new source the create returned; one write.
        unsafe { obs_source_duplicate_entry_set_audio_mixers(s.as_ptr(), value) }
    }
    fn sync_offset(&self) -> i64 {
        // SAFETY: a live source; the getter locks and reads.
        unsafe { obs_source_duplicate_entry_sync_offset(self.source.as_ptr()) }
    }
    fn set_sync_offset(&self, s: NonNull<c_void>, value: i64) {
        // SAFETY: the new source; one write.
        unsafe { obs_source_duplicate_entry_set_sync_offset(s.as_ptr(), value) }
    }
    fn user_volume(&self) -> f32 {
        // SAFETY: a live source; the getter locks and reads.
        unsafe { obs_source_duplicate_entry_user_volume(self.source.as_ptr()) }
    }
    fn set_user_volume(&self, s: NonNull<c_void>, value: f32) {
        // SAFETY: the new source; one write.
        unsafe { obs_source_duplicate_entry_set_user_volume(s.as_ptr(), value) }
    }
    fn user_muted(&self) -> bool {
        // SAFETY: a live source; one atomic load.
        unsafe { obs_source_duplicate_entry_user_muted(self.source.as_ptr()) }
    }
    fn set_user_muted(&self, s: NonNull<c_void>, value: bool) {
        // SAFETY: the new source; one atomic store.
        unsafe { obs_source_duplicate_entry_set_user_muted(s.as_ptr(), value) }
    }
    fn volume(&self) -> f32 {
        // SAFETY: a live source; one read.
        unsafe { obs_source_duplicate_entry_volume(self.source.as_ptr()) }
    }
    fn set_volume(&self, s: NonNull<c_void>, value: f32) {
        // SAFETY: the new source; one write.
        unsafe { obs_source_duplicate_entry_set_volume(s.as_ptr(), value) }
    }
    fn muted(&self) -> bool {
        // SAFETY: a live source; one read.
        unsafe { obs_source_duplicate_entry_muted(self.source.as_ptr()) }
    }
    fn set_muted(&self, s: NonNull<c_void>, value: bool) {
        // SAFETY: the new source; one write.
        unsafe { obs_source_duplicate_entry_set_muted(s.as_ptr(), value) }
    }
    fn flags(&self) -> u32 {
        // SAFETY: a live source; one read.
        unsafe { obs_source_duplicate_entry_flags(self.source.as_ptr()) }
    }
    fn set_flags(&self, s: NonNull<c_void>, value: u32) {
        // SAFETY: the new source; one write.
        unsafe { obs_source_duplicate_entry_set_flags(s.as_ptr(), value) }
    }
    fn private_settings_apply(&self, s: NonNull<c_void>) {
        // SAFETY: the new source and the live source, both with private
        // settings objects.
        unsafe {
            obs_source_duplicate_entry_private_settings_apply(s.as_ptr(), self.source.as_ptr())
        }
    }
    fn duplicate_filters(&self, s: NonNull<c_void>, private: bool) {
        // SAFETY: the new source and the live source.
        unsafe { obs_source_duplicate_entry_filters(s.as_ptr(), self.source.as_ptr(), private) }
    }
}

fn null_log(function: &'static CStr, parameter: &'static CStr) {
    // SAFETY: static strings match the two %s arguments of the format.
    unsafe {
        blog(
            400,
            c"%s: Null '%s' parameter".as_ptr(),
            function.as_ptr(),
            parameter.as_ptr(),
        );
    }
}

/// # Safety
/// A non-NULL source must be a source under the original libobs contract,
/// alive for the call; `new_name` is passed on to the create or scene copy
/// as the original passes it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_duplicate(
    source: *mut c_void,
    new_name: *const c_char,
    create_private: bool,
) -> *mut c_void {
    // No adapter runs until NULL has been rejected.
    let live = NonNull::new(source).map(|source| Live { source, new_name });
    duplicate(live.as_ref(), create_private, null_log).map_or(ptr::null_mut(), NonNull::as_ptr)
}
