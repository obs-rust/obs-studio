//! Tier 1 mirrors of test_source_flags_mixers.c without FFI or unsafe code,
//! on a model source that records every access in order.
use core::ffi::CStr;
use obs_source::flags_mixers::{
    SET_AUDIO_MIXERS_FUNCTION, SET_FLAGS_FUNCTION, Source, set_audio_mixers, set_flags,
};
use obs_source::monitoring::AUDIO;
use std::cell::{Cell, RefCell};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Flags,
    StoreFlags(u32),
    /// `update_flags`, with the flags field at the time.
    UpdateFlags(u32),
    OwnsInfoId,
    OutputFlags,
    Mixers,
    /// `audio_mixers`, with the value sent and the mixers field at the time.
    AudioMixers(i64, u32),
    StoreMixers(u32),
}

/// A model source; `leave` is what the `audio_mixers` handlers leave in the
/// calldata (`None`: what was sent).
struct Model {
    flags: Cell<u32>,
    owns_info_id: bool,
    output_flags: u32,
    mixers: Cell<u32>,
    leave: Option<i64>,
    events: RefCell<Vec<Event>>,
}

impl Model {
    fn new(output_flags: u32, owns_info_id: bool, flags: u32, mixers: u32) -> Self {
        Self {
            flags: Cell::new(flags),
            owns_info_id,
            output_flags,
            mixers: Cell::new(mixers),
            leave: None,
            events: RefCell::new(Vec::new()),
        }
    }
    fn push(&self, event: Event) {
        self.events.borrow_mut().push(event);
    }
    fn take(&self) -> Vec<Event> {
        self.events.take()
    }
}

impl Source for Model {
    fn flags(&self) -> u32 {
        self.push(Event::Flags);
        self.flags.get()
    }
    fn store_flags(&self, flags: u32) {
        self.push(Event::StoreFlags(flags));
        self.flags.set(flags);
    }
    fn signal_update_flags(&self) {
        self.push(Event::UpdateFlags(self.flags.get()));
    }
    fn owns_info_id(&self) -> bool {
        self.push(Event::OwnsInfoId);
        self.owns_info_id
    }
    fn output_flags(&self) -> u32 {
        self.push(Event::OutputFlags);
        self.output_flags
    }
    fn audio_mixers(&self) -> u32 {
        self.push(Event::Mixers);
        self.mixers.get()
    }
    fn signal_audio_mixers(&self, mixers: i64) -> i64 {
        self.push(Event::AudioMixers(mixers, self.mixers.get()));
        self.leave.unwrap_or(mixers)
    }
    fn store_audio_mixers(&self, mixers: u32) {
        self.push(Event::StoreMixers(mixers));
        self.mixers.set(mixers);
    }
}

fn no_log(function: &'static CStr) {
    panic!("logged {function:?}");
}

#[test]
fn null_source_logs_once() {
    let mut logs = Vec::new();
    set_flags(None::<&Model>, 1, |f| logs.push(f));
    set_audio_mixers(None::<&Model>, 1, |f| logs.push(f));
    assert_eq!(logs, [SET_FLAGS_FUNCTION, SET_AUDIO_MIXERS_FUNCTION]);
    assert_eq!(SET_FLAGS_FUNCTION, c"obs_source_set_flags");
    assert_eq!(SET_AUDIO_MIXERS_FUNCTION, c"obs_source_set_audio_mixers");
}

#[test]
fn set_flags_stores_then_signals_a_change() {
    const FORCE_MONO: u32 = 1 << 1;
    let values = [0, 0, FORCE_MONO, FORCE_MONO, u32::MAX, 0x5a5a_5a5a, 0];
    for initial in [0, u32::MAX, 0x5a5a_5a5a] {
        let model = Model::new(AUDIO, false, initial, 7);
        let mut flags = initial;
        for value in values {
            set_flags(Some(&model), value, no_log);
            if value == flags {
                assert_eq!(model.take(), [Event::Flags]);
            } else {
                assert_eq!(
                    model.take(),
                    [
                        Event::Flags,
                        Event::StoreFlags(value),
                        Event::UpdateFlags(value)
                    ]
                );
                flags = value;
            }
            assert_eq!(model.flags.get(), flags);
            assert_eq!(model.mixers.get(), 7);
        }
    }
}

#[test]
fn set_audio_mixers_needs_audio_or_an_owned_id() {
    for output_flags in [0, 1, 1 << 2, !AUDIO] {
        for initial in [0, u32::MAX, 0x5a5a_5a5a] {
            let model = Model::new(output_flags, false, 9, initial);
            for value in [initial ^ 1, 0, 0x3f] {
                set_audio_mixers(Some(&model), value, no_log);
                assert_eq!(model.take(), [Event::OwnsInfoId, Event::OutputFlags]);
            }
            assert_eq!(model.mixers.get(), initial);

            // Owning the info id is enough, and the flags are then not read.
            let model = Model::new(output_flags, true, 9, initial);
            set_audio_mixers(Some(&model), initial ^ 1, no_log);
            assert_eq!(
                model.take(),
                [
                    Event::OwnsInfoId,
                    Event::Mixers,
                    Event::AudioMixers(i64::from(initial ^ 1), initial),
                    Event::StoreMixers(initial ^ 1)
                ]
            );
            assert_eq!(model.flags.get(), 9);
        }
    }
}

#[test]
fn set_audio_mixers_signals_before_the_store() {
    let values = [0x3f, 0x3f, 0x5, 0, 0, u32::MAX, 1];
    for initial in [0, u32::MAX, 0x5a5a_5a5a] {
        let model = Model::new(AUDIO | 1, false, 9, initial);
        let mut mixers = initial;
        for value in values {
            set_audio_mixers(Some(&model), value, no_log);
            let mut expected = vec![Event::OwnsInfoId, Event::OutputFlags, Event::Mixers];
            if value != mixers {
                // The handlers still see the old value.
                expected.extend([
                    Event::AudioMixers(i64::from(value), mixers),
                    Event::StoreMixers(value),
                ]);
                mixers = value;
            }
            assert_eq!(model.take(), expected);
            assert_eq!(model.mixers.get(), mixers);
            assert_eq!(model.flags.get(), 9);
        }
    }
}

#[test]
fn set_audio_mixers_stores_what_the_handlers_leave() {
    let leaves: [(i64, u32); 9] = [
        (0, 0),
        (0x3f, 0x3f),
        (0xffff_ffff, u32::MAX),
        (0x1_0000_0005, 5),
        (-1, u32::MAX),
        (-0x1_0000_0000, 0),
        (i64::MIN, 0),
        (i64::MAX, u32::MAX),
        (0x107, 0x107),
    ];
    for (leave, stored) in leaves {
        let mut model = Model::new(AUDIO, false, 9, 4);
        model.leave = Some(leave);
        set_audio_mixers(Some(&model), 6, no_log);
        assert_eq!(
            model.take(),
            [
                Event::OwnsInfoId,
                Event::OutputFlags,
                Event::Mixers,
                Event::AudioMixers(6, 4),
                Event::StoreMixers(stored)
            ]
        );
        assert_eq!(model.mixers.get(), stored);
    }
}
