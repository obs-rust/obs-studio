//! `obs_source_duplicate()`: the duplicate behind the frontend's Duplicate
//! (sources, transitions and filter paste), `obs_scene_duplicate()`'s item
//! copies and `obs_source_copy_filters()`.
//!
//! It runs the decisions of [`crate::source_duplicate`] in the C's order,
//! one after each lookup or read. The C around it stays behind [`Source`]:
//! the struct reads and writes, the atomic `user_muted` access, the scene
//! and group lookups, `obs_scene_duplicate()` and `obs_scene_get_source()`,
//! the creates, the `obs_data_*` calls and `obs_source_duplicate_filters()`.
#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

use crate::source_duplicate::{
    Step, copies_filters, created_step, flags_step, group_step, scene_step, scene_type, type_step,
};
use core::ffi::CStr;

/// The public function named in the NULL diagnostic.
pub const DUPLICATE_FUNCTION: &CStr = c"obs_source_duplicate";
/// The parameter a NULL source is logged as.
pub const SOURCE_PARAMETER: &CStr = c"source";

/// What the duplicate reads, writes and calls, in the order it asks. `self`
/// is the source passed in, with the new name the caller gave;
/// [`Self::Ref`] is a source it returns (a new reference, a new source or a
/// scene copy's source) or copies state into.
pub trait Source {
    /// A source handle.
    type Ref: Copy;
    /// A scene (or group) handle.
    type Scene: Copy;
    /// A settings object handle.
    type Settings: Copy;
    /// `source->info.type`.
    fn source_type(&self) -> i32;
    /// `obs_scene_from_source(source)`; `None` for NULL.
    fn scene(&self) -> Option<Self::Scene>;
    /// `obs_group_from_source(source)`; `None` for NULL.
    fn group(&self) -> Option<Self::Scene>;
    /// `obs_source_get_ref(source)`; `None` for NULL.
    fn get_ref(&self) -> Option<Self::Ref>;
    /// `obs_scene_duplicate(scene, new_name, dup_type)`; `None` for NULL.
    fn scene_duplicate(&self, scene: Self::Scene, dup_type: i32) -> Option<Self::Scene>;
    /// `obs_scene_get_source(scene)`, NULL passed as NULL.
    fn scene_source(&self, scene: Option<Self::Scene>) -> Option<Self::Ref>;
    /// `source->info.output_flags`.
    fn output_flags(&self) -> u32;
    /// `obs_data_create()`.
    fn settings_create(&self) -> Self::Settings;
    /// `obs_data_apply(settings, source->context.settings)`.
    fn settings_apply(&self, settings: Self::Settings);
    /// `obs_source_create_private(source->info.id, new_name, settings)`, or
    /// for a public one `obs_source_create(source->info.id, new_name,
    /// settings, NULL)`; `None` for NULL.
    fn create(&self, settings: Self::Settings, private: bool) -> Option<Self::Ref>;
    /// `obs_data_release(settings)`.
    fn settings_release(&self, settings: Self::Settings);
    /// `source->audio_mixers`.
    fn audio_mixers(&self) -> u32;
    /// `s->audio_mixers = value`.
    fn set_audio_mixers(&self, s: Self::Ref, value: u32);
    /// `obs_source_get_sync_offset(source)`.
    fn sync_offset(&self) -> i64;
    /// `s->sync_offset = value`.
    fn set_sync_offset(&self, s: Self::Ref, value: i64);
    /// `obs_source_get_volume(source)`.
    fn user_volume(&self) -> f32;
    /// `s->user_volume = value`.
    fn set_user_volume(&self, s: Self::Ref, value: f32);
    /// `os_atomic_load_bool(&source->user_muted)`.
    fn user_muted(&self) -> bool;
    /// `os_atomic_set_bool(&s->user_muted, value)`.
    fn set_user_muted(&self, s: Self::Ref, value: bool);
    /// `source->volume`.
    fn volume(&self) -> f32;
    /// `s->volume = value`.
    fn set_volume(&self, s: Self::Ref, value: f32);
    /// `source->muted`.
    fn muted(&self) -> bool;
    /// `s->muted = value`.
    fn set_muted(&self, s: Self::Ref, value: bool);
    /// `source->flags`.
    fn flags(&self) -> u32;
    /// `s->flags = value`.
    fn set_flags(&self, s: Self::Ref, value: u32);
    /// `obs_data_apply(s->private_settings, source->private_settings)`.
    fn private_settings_apply(&self, s: Self::Ref);
    /// `obs_source_duplicate_filters(s, source, private)`.
    fn duplicate_filters(&self, s: Self::Ref, private: bool);
}

/// `obs_source_duplicate()`: `None` is a NULL source, logged as
/// ([`DUPLICATE_FUNCTION`], [`SOURCE_PARAMETER`]), and gives `None`.
///
/// A scene-typed source follows [`crate::source_duplicate::scene_step`] and
/// [`crate::source_duplicate::group_step`]: a new reference, `None`, or the
/// source of the scene's or group's copy of the type
/// [`crate::source_duplicate::scene_type`] gives (no extra reference). A
/// source marked `OBS_SOURCE_DO_NOT_DUPLICATE` gives a new reference. Any
/// other is created from a copy of its settings; without a new source the
/// settings are released and the answer is `None`. With one, the audio
/// mixers, sync offset, user volume, user mute, volume, mute and flags are
/// copied, the private settings applied, the filters duplicated unless it
/// is a filter, and the settings released.
pub fn duplicate<S: Source>(
    source: Option<&S>,
    create_private: bool,
    log: impl FnOnce(&'static CStr, &'static CStr),
) -> Option<S::Ref> {
    let Some(source) = source else {
        log(DUPLICATE_FUNCTION, SOURCE_PARAMETER);
        return None;
    };
    if type_step(source.source_type()) == Step::AskScene {
        let mut scene = source.scene();
        let mut step = scene_step(scene.is_some(), create_private);
        if step == Step::Ref {
            return source.get_ref();
        }
        if step == Step::AskGroup {
            scene = source.group();
            step = group_step(scene.is_some());
        }
        let (Step::SceneCopy, Some(scene)) = (step, scene) else {
            return None;
        };
        let new_scene = source.scene_duplicate(scene, scene_type(create_private));
        return source.scene_source(new_scene);
    }

    if flags_step(source.output_flags()) == Step::Ref {
        return source.get_ref();
    }

    let settings = source.settings_create();
    source.settings_apply(settings);
    let new_source = source.create(settings, create_private);
    let (Step::CopyState, Some(s)) = (created_step(new_source.is_some()), new_source) else {
        source.settings_release(settings);
        return None;
    };

    source.set_audio_mixers(s, source.audio_mixers());
    source.set_sync_offset(s, source.sync_offset());
    source.set_user_volume(s, source.user_volume());
    source.set_user_muted(s, source.user_muted());
    source.set_volume(s, source.volume());
    source.set_muted(s, source.muted());
    source.set_flags(s, source.flags());

    source.private_settings_apply(s);
    // The type is read again here, as the C reads it.
    if copies_filters(source.source_type()) {
        source.duplicate_filters(s, create_private);
    }

    source.settings_release(settings);
    Some(s)
}
