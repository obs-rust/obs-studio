//! Tier 1 mirrors of test_source_update.c over a model source: the stored
//! and incoming settings are maps, `obs_data_apply()` merges one into the
//! other, the plugin's update callback sees the stored map, and the signal
//! is recorded. Every read and call is recorded in order.
use core::cell::RefCell;
use core::ffi::CStr;
use obs_source::update::{OBS_SOURCE_VIDEO, Source, update};
use obs_util::platform::CLongAtomic;
use std::collections::BTreeMap;
use std::sync::atomic::Ordering::SeqCst;

type Settings = BTreeMap<&'static str, i64>;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Log(&'static CStr),
    Apply,
    Flags,
    Defer,
    CanUpdate,
    Update(Settings),
    Signal,
}

struct Model {
    settings: RefCell<Settings>,
    incoming: Option<Settings>,
    flags: u32,
    has_data: bool,
    has_update: bool,
    defer: CLongAtomic,
    events: RefCell<Vec<Event>>,
}

impl Model {
    fn new(flags: u32, has_data: bool, has_update: bool) -> Self {
        Self {
            settings: RefCell::new(settings(&[("a", 1), ("b", 1)])),
            incoming: None,
            flags,
            has_data,
            has_update,
            defer: CLongAtomic::new(0),
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
    fn apply_settings(&self) {
        self.push(Event::Apply);
        let incoming = self.incoming.as_ref().expect("applied NULL settings");
        self.settings
            .borrow_mut()
            .extend(incoming.iter().map(|(k, v)| (*k, *v)));
    }
    fn output_flags(&self) -> u32 {
        self.push(Event::Flags);
        self.flags
    }
    fn defer_update_count(&self) -> &CLongAtomic {
        self.push(Event::Defer);
        &self.defer
    }
    fn can_update(&self) -> bool {
        self.push(Event::CanUpdate);
        self.has_data && self.has_update
    }
    fn call_update(&self) {
        assert!(self.has_data && self.has_update, "called a missing update");
        self.push(Event::Update(self.settings.borrow().clone()));
    }
    fn signal_update(&self) {
        self.push(Event::Signal);
    }
}

fn settings(pairs: &[(&'static str, i64)]) -> Settings {
    pairs.iter().copied().collect()
}

fn run(mut source: Model, incoming: Option<Settings>) -> Vec<Event> {
    let has_settings = incoming.is_some();
    source.incoming = incoming;
    update(Some(&source), has_settings, |name| {
        source.push(Event::Log(name));
    });
    source.take()
}

#[test]
fn null_source_logs_and_reads_nothing() {
    let mut logs = Vec::new();
    update(None::<&Model>, true, |name| logs.push(name));
    update(None::<&Model>, false, |name| logs.push(name));
    assert_eq!(logs, [c"obs_source_update", c"obs_source_update"]);
}

#[test]
fn sync_update_merges_then_calls_then_signals() {
    let incoming = settings(&[("b", 2), ("c", 3)]);
    let merged = settings(&[("a", 1), ("b", 2), ("c", 3)]);
    assert_eq!(
        run(Model::new(0, true, true), Some(incoming)),
        [
            Event::Apply,
            Event::Flags,
            Event::CanUpdate,
            Event::Update(merged),
            Event::Signal
        ]
    );
}

#[test]
fn null_settings_still_dispatch_without_merge() {
    assert_eq!(
        run(Model::new(0, true, true), None),
        [
            Event::Flags,
            Event::CanUpdate,
            Event::Update(settings(&[("a", 1), ("b", 1)])),
            Event::Signal
        ]
    );
}

#[test]
fn no_callback_or_no_data_merges_without_dispatch() {
    for (has_data, has_update) in [(true, false), (false, true), (false, false)] {
        assert_eq!(
            run(
                Model::new(0, has_data, has_update),
                Some(settings(&[("c", 3)]))
            ),
            [Event::Apply, Event::Flags, Event::CanUpdate]
        );
        assert_eq!(
            run(Model::new(0, has_data, has_update), None),
            [Event::Flags, Event::CanUpdate]
        );
    }
}

#[test]
fn video_sources_only_defer() {
    // Any flag word with OBS_SOURCE_VIDEO set defers, whatever else is set
    // and whether or not the source could update now.
    for flags in [OBS_SOURCE_VIDEO, u32::MAX, OBS_SOURCE_VIDEO | 1 << 2] {
        for (has_data, has_update) in [(true, true), (false, false)] {
            let model = Model {
                incoming: Some(settings(&[("c", 3)])),
                ..Model::new(flags, has_data, has_update)
            };
            update(Some(&model), true, |_| panic!("logged"));
            assert_eq!(model.take(), [Event::Apply, Event::Flags, Event::Defer]);
            assert_eq!(model.defer.load(SeqCst), 1);
            update(Some(&model), false, |_| panic!("logged"));
            assert_eq!(model.take(), [Event::Flags, Event::Defer]);
            assert_eq!(model.defer.load(SeqCst), 2);
        }
    }
}

#[test]
fn video_defer_count_keeps_counting_from_any_value() {
    // Characterized, not endorsed: the count is a plain increment, so a
    // count already below zero (or at the top of a long) moves on as C's
    // os_atomic_inc_long() would.
    for start in [-5, -1, 0, 41] {
        let model = Model::new(OBS_SOURCE_VIDEO, true, true);
        model.defer.store(start, SeqCst);
        update(Some(&model), false, |_| panic!("logged"));
        assert_eq!(model.defer.load(SeqCst), start.wrapping_add(1));
    }
}

#[test]
fn non_video_flags_never_defer() {
    for flags in [0, 1 << 1, 1 << 2, !OBS_SOURCE_VIDEO] {
        let model = Model::new(flags, true, true);
        update(Some(&model), false, |_| panic!("logged"));
        assert_eq!(
            model.take(),
            [
                Event::Flags,
                Event::CanUpdate,
                Event::Update(settings(&[("a", 1), ("b", 1)])),
                Event::Signal
            ]
        );
        assert_eq!(model.defer.load(SeqCst), 0);
    }
}
