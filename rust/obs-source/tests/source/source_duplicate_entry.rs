//! Tier 1 mirrors of test_source_duplicate.c without FFI or unsafe code.
use core::ffi::CStr;
use obs_source::source_duplicate::{
    DO_NOT_DUPLICATE, DUP_COPY, DUP_PRIVATE_COPY, TYPE_FILTER, TYPE_SCENE,
};
use obs_source::source_duplicate_entry::{DUPLICATE_FUNCTION, SOURCE_PARAMETER, Source, duplicate};
use std::cell::RefCell;

/// `OBS_SOURCE_TYPE_INPUT`.
const TYPE_INPUT: i32 = 0;
/// `OBS_SOURCE_TYPE_TRANSITION`.
const TYPE_TRANSITION: i32 = 2;

/// Source handles: the source passed in, a reference to it, a new source,
/// a scene copy's source.
const REF: u32 = 1;
const CREATED: u32 = 2;
const COPY_SOURCE: u32 = 3;
/// Scene handles: the scene found, the group found, the copy.
const SCENE: u32 = 10;
const GROUP: u32 = 11;
const SCENE_COPY: u32 = 12;
/// The settings object created.
const SETTINGS: u32 = 20;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Event {
    Type,
    Scene,
    Group,
    GetRef,
    SceneDuplicate(u32, i32),
    SceneSource(Option<u32>),
    OutputFlags,
    SettingsCreate,
    SettingsApply(u32),
    Create(u32, bool),
    SettingsRelease(u32),
    AudioMixers,
    SetAudioMixers(u32, u32),
    SyncOffset,
    SetSyncOffset(u32, i64),
    UserVolume,
    /// The bits, so NaN and -0.0 compare exactly.
    SetUserVolume(u32, u32),
    UserMuted,
    SetUserMuted(u32, bool),
    Volume,
    SetVolume(u32, u32),
    Muted,
    SetMuted(u32, bool),
    Flags,
    SetFlags(u32, u32),
    PrivateSettingsApply(u32),
    DuplicateFilters(u32, bool),
}

/// A model source: what each read and lookup answers; every call recorded.
struct Model {
    source_type: i32,
    output_flags: u32,
    scene: Option<u32>,
    group: Option<u32>,
    reference: Option<u32>,
    scene_copy: Option<u32>,
    scene_source: Option<u32>,
    created: Option<u32>,
    audio_mixers: u32,
    sync_offset: i64,
    user_volume: f32,
    user_muted: bool,
    volume: f32,
    muted: bool,
    flags: u32,
    events: RefCell<Vec<Event>>,
}

impl Model {
    fn new(source_type: i32) -> Self {
        Self {
            source_type,
            output_flags: 0,
            scene: None,
            group: None,
            reference: Some(REF),
            scene_copy: Some(SCENE_COPY),
            scene_source: Some(COPY_SOURCE),
            created: Some(CREATED),
            audio_mixers: 0x2d,
            sync_offset: -123_456_789,
            user_volume: 0.25,
            user_muted: true,
            volume: 0.75,
            muted: true,
            flags: 5,
            events: RefCell::new(Vec::new()),
        }
    }
    fn push(&self, event: Event) {
        self.events.borrow_mut().push(event);
    }
    fn events(&self) -> Vec<Event> {
        self.events.borrow().clone()
    }
}

impl Source for Model {
    type Ref = u32;
    type Scene = u32;
    type Settings = u32;
    fn source_type(&self) -> i32 {
        self.push(Event::Type);
        self.source_type
    }
    fn scene(&self) -> Option<u32> {
        self.push(Event::Scene);
        self.scene
    }
    fn group(&self) -> Option<u32> {
        self.push(Event::Group);
        self.group
    }
    fn get_ref(&self) -> Option<u32> {
        self.push(Event::GetRef);
        self.reference
    }
    fn scene_duplicate(&self, scene: u32, dup_type: i32) -> Option<u32> {
        self.push(Event::SceneDuplicate(scene, dup_type));
        self.scene_copy
    }
    fn scene_source(&self, scene: Option<u32>) -> Option<u32> {
        self.push(Event::SceneSource(scene));
        scene.and(self.scene_source)
    }
    fn output_flags(&self) -> u32 {
        self.push(Event::OutputFlags);
        self.output_flags
    }
    fn settings_create(&self) -> u32 {
        self.push(Event::SettingsCreate);
        SETTINGS
    }
    fn settings_apply(&self, settings: u32) {
        self.push(Event::SettingsApply(settings));
    }
    fn create(&self, settings: u32, private: bool) -> Option<u32> {
        self.push(Event::Create(settings, private));
        self.created
    }
    fn settings_release(&self, settings: u32) {
        self.push(Event::SettingsRelease(settings));
    }
    fn audio_mixers(&self) -> u32 {
        self.push(Event::AudioMixers);
        self.audio_mixers
    }
    fn set_audio_mixers(&self, s: u32, value: u32) {
        self.push(Event::SetAudioMixers(s, value));
    }
    fn sync_offset(&self) -> i64 {
        self.push(Event::SyncOffset);
        self.sync_offset
    }
    fn set_sync_offset(&self, s: u32, value: i64) {
        self.push(Event::SetSyncOffset(s, value));
    }
    fn user_volume(&self) -> f32 {
        self.push(Event::UserVolume);
        self.user_volume
    }
    fn set_user_volume(&self, s: u32, value: f32) {
        self.push(Event::SetUserVolume(s, value.to_bits()));
    }
    fn user_muted(&self) -> bool {
        self.push(Event::UserMuted);
        self.user_muted
    }
    fn set_user_muted(&self, s: u32, value: bool) {
        self.push(Event::SetUserMuted(s, value));
    }
    fn volume(&self) -> f32 {
        self.push(Event::Volume);
        self.volume
    }
    fn set_volume(&self, s: u32, value: f32) {
        self.push(Event::SetVolume(s, value.to_bits()));
    }
    fn muted(&self) -> bool {
        self.push(Event::Muted);
        self.muted
    }
    fn set_muted(&self, s: u32, value: bool) {
        self.push(Event::SetMuted(s, value));
    }
    fn flags(&self) -> u32 {
        self.push(Event::Flags);
        self.flags
    }
    fn set_flags(&self, s: u32, value: u32) {
        self.push(Event::SetFlags(s, value));
    }
    fn private_settings_apply(&self, s: u32) {
        self.push(Event::PrivateSettingsApply(s));
    }
    fn duplicate_filters(&self, s: u32, private: bool) {
        self.push(Event::DuplicateFilters(s, private));
    }
}

fn no_log(function: &'static CStr, parameter: &'static CStr) {
    panic!("logged {function:?}: {parameter:?}");
}

/// The events of a create that returned a source, up to the filter check,
/// for the model's default state.
fn copied(private: bool) -> Vec<Event> {
    vec![
        Event::Type,
        Event::OutputFlags,
        Event::SettingsCreate,
        Event::SettingsApply(SETTINGS),
        Event::Create(SETTINGS, private),
        Event::AudioMixers,
        Event::SetAudioMixers(CREATED, 0x2d),
        Event::SyncOffset,
        Event::SetSyncOffset(CREATED, -123_456_789),
        Event::UserVolume,
        Event::SetUserVolume(CREATED, 0.25f32.to_bits()),
        Event::UserMuted,
        Event::SetUserMuted(CREATED, true),
        Event::Volume,
        Event::SetVolume(CREATED, 0.75f32.to_bits()),
        Event::Muted,
        Event::SetMuted(CREATED, true),
        Event::Flags,
        Event::SetFlags(CREATED, 5),
        Event::PrivateSettingsApply(CREATED),
        // The type read again for the filter check.
        Event::Type,
    ]
}

#[test]
fn null_source_logs_once_and_reads_nothing() {
    for private in [false, true] {
        let mut logged = Vec::new();
        let got = duplicate::<Model>(None, private, |function, parameter| {
            logged.push((function, parameter));
        });
        assert_eq!(got, None);
        assert_eq!(logged, [(DUPLICATE_FUNCTION, SOURCE_PARAMETER)]);
    }
    assert_eq!(DUPLICATE_FUNCTION, c"obs_source_duplicate");
    assert_eq!(SOURCE_PARAMETER, c"source");
}

#[test]
fn public_scene_is_shared() {
    for reference in [Some(REF), None] {
        let mut model = Model::new(TYPE_SCENE);
        model.scene = Some(SCENE);
        model.reference = reference;
        assert_eq!(duplicate(Some(&model), false, no_log), reference);
        assert_eq!(model.events(), [Event::Type, Event::Scene, Event::GetRef]);
    }
}

#[test]
fn private_scene_is_copied() {
    let mut model = Model::new(TYPE_SCENE);
    model.scene = Some(SCENE);
    assert_eq!(duplicate(Some(&model), true, no_log), Some(COPY_SOURCE));
    assert_eq!(
        model.events(),
        [
            Event::Type,
            Event::Scene,
            Event::SceneDuplicate(SCENE, DUP_PRIVATE_COPY),
            Event::SceneSource(Some(SCENE_COPY)),
        ]
    );
}

#[test]
fn group_is_copied_public_or_private() {
    for (private, dup_type) in [(false, DUP_COPY), (true, DUP_PRIVATE_COPY)] {
        let mut model = Model::new(TYPE_SCENE);
        model.group = Some(GROUP);
        assert_eq!(duplicate(Some(&model), private, no_log), Some(COPY_SOURCE));
        assert_eq!(
            model.events(),
            [
                Event::Type,
                Event::Scene,
                Event::Group,
                Event::SceneDuplicate(GROUP, dup_type),
                Event::SceneSource(Some(SCENE_COPY)),
            ]
        );
    }
}

#[test]
fn failed_scene_copy_asks_its_source_of_null() {
    let mut model = Model::new(TYPE_SCENE);
    model.scene = Some(SCENE);
    model.scene_copy = None;
    assert_eq!(duplicate(Some(&model), true, no_log), None);
    assert_eq!(
        model.events(),
        [
            Event::Type,
            Event::Scene,
            Event::SceneDuplicate(SCENE, DUP_PRIVATE_COPY),
            Event::SceneSource(None),
        ]
    );
}

#[test]
fn scene_typed_source_that_is_neither_has_no_duplicate() {
    for private in [false, true] {
        let model = Model::new(TYPE_SCENE);
        assert_eq!(duplicate(Some(&model), private, no_log), None);
        assert_eq!(model.events(), [Event::Type, Event::Scene, Event::Group]);
    }
}

#[test]
fn do_not_duplicate_is_shared() {
    for source_type in [TYPE_INPUT, TYPE_FILTER, TYPE_TRANSITION] {
        for private in [false, true] {
            for reference in [Some(REF), None] {
                let mut model = Model::new(source_type);
                model.output_flags = DO_NOT_DUPLICATE | 1;
                model.reference = reference;
                assert_eq!(duplicate(Some(&model), private, no_log), reference);
                assert_eq!(
                    model.events(),
                    [Event::Type, Event::OutputFlags, Event::GetRef]
                );
            }
        }
    }
}

#[test]
fn source_is_created_with_its_state_and_filters() {
    for source_type in [TYPE_INPUT, TYPE_TRANSITION, 4, -1] {
        for private in [false, true] {
            let mut model = Model::new(source_type);
            model.output_flags = !DO_NOT_DUPLICATE;
            assert_eq!(duplicate(Some(&model), private, no_log), Some(CREATED));
            let mut expected = copied(private);
            expected.push(Event::DuplicateFilters(CREATED, private));
            expected.push(Event::SettingsRelease(SETTINGS));
            assert_eq!(model.events(), expected);
        }
    }
}

#[test]
fn filter_is_created_without_filters() {
    for private in [false, true] {
        let model = Model::new(TYPE_FILTER);
        assert_eq!(duplicate(Some(&model), private, no_log), Some(CREATED));
        let mut expected = copied(private);
        expected.push(Event::SettingsRelease(SETTINGS));
        assert_eq!(model.events(), expected);
    }
}

#[test]
fn failed_create_releases_the_settings() {
    for source_type in [TYPE_INPUT, TYPE_FILTER] {
        for private in [false, true] {
            let mut model = Model::new(source_type);
            model.created = None;
            assert_eq!(duplicate(Some(&model), private, no_log), None);
            assert_eq!(
                model.events(),
                [
                    Event::Type,
                    Event::OutputFlags,
                    Event::SettingsCreate,
                    Event::SettingsApply(SETTINGS),
                    Event::Create(SETTINGS, private),
                    Event::SettingsRelease(SETTINGS),
                ]
            );
        }
    }
}

#[test]
fn copied_values_pass_unchanged() {
    for (volume, user_volume) in [(f32::NAN, -0.0), (f32::INFINITY, f32::MIN_POSITIVE)] {
        let mut model = Model::new(TYPE_INPUT);
        model.audio_mixers = u32::MAX;
        model.sync_offset = i64::MIN;
        model.user_volume = user_volume;
        model.user_muted = false;
        model.volume = volume;
        model.muted = false;
        model.flags = 0;
        assert_eq!(duplicate(Some(&model), false, no_log), Some(CREATED));
        let events = model.events();
        for event in [
            Event::SetAudioMixers(CREATED, u32::MAX),
            Event::SetSyncOffset(CREATED, i64::MIN),
            Event::SetUserVolume(CREATED, user_volume.to_bits()),
            Event::SetUserMuted(CREATED, false),
            Event::SetVolume(CREATED, volume.to_bits()),
            Event::SetMuted(CREATED, false),
            Event::SetFlags(CREATED, 0),
        ] {
            assert!(events.contains(&event), "{event:?} in {events:?}");
        }
    }
}
