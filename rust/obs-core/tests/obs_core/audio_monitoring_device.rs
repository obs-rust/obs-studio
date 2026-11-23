//! Safe mirrors of test_set_audio_monitoring_device.c (and the refusals of
//! test_core_setup_rules_steps' test_monitoring_device) over a model core:
//! a stored name and id it owns, live monitors that remember the device each
//! reset put them on, and public and private sources with output flags and a
//! `device_id` setting, whose notices drive a model of the deduplication
//! signal (a capture of `<id>.monitor` starts it, the deduplicating source
//! that no longer matches stops it). Plus what the C test cannot see: the
//! refusals come before any other step, availability next, then the lock,
//! the compare, the copies, the unlock, the resets under their own lock and
//! one notice per Audio Output Capture source in enumeration order.
use obs_core::audio_monitoring_device::{
    Device, Monitors, aoc_source, reset_monitoring, set_device,
};
use obs_core::core_setup_rules::DO_NOT_SELF_MONITOR;

const AUDIO: u32 = 1 << 1;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Step {
    Empty(String),
    Available,
    Lock,
    Unlock,
    Compare(String),
    Store(String, String),
    Count,
    Reset(usize),
    Enum,
    Flags(usize),
    Notify(usize, String),
}

struct Source {
    flags: u32,
    device_id: &'static str,
    public: bool,
}

struct Model {
    available: bool,
    name: String,
    id: String,
    locked: bool,
    /// The device each monitor was last reset onto, and how often.
    monitors: Vec<(String, usize)>,
    sources: Vec<Source>,
    duplicating: Option<usize>,
    signals: Vec<Option<usize>>,
    steps: Vec<Step>,
}

impl Model {
    fn new() -> Self {
        Self {
            available: true,
            name: "Default".into(),
            id: "default".into(),
            locked: false,
            monitors: Vec::new(),
            sources: Vec::new(),
            duplicating: None,
            signals: Vec::new(),
            steps: Vec::new(),
        }
    }

    fn with_monitors(count: usize) -> Self {
        let mut model = Self::new();
        model.monitors = vec![(String::new(), 0); count];
        model
    }

    /// The caller's strings live on (leaked, a few bytes per call), as a C
    /// caller's do for the call; the model stores its own copies.
    fn set(&mut self, name: Option<&str>, id: Option<&str>) -> bool {
        set_device(self, name.map(leak), id.map(leak))
    }

    fn device(&self) -> (&str, &str) {
        (&self.name, &self.id)
    }

    fn resets(&self) -> Vec<usize> {
        self.monitors.iter().map(|(_, n)| *n).collect()
    }

    fn take_steps(&mut self) -> Vec<Step> {
        core::mem::take(&mut self.steps)
    }
}

impl Monitors for Model {
    fn available(&mut self) -> bool {
        self.steps.push(Step::Available);
        self.available
    }
    fn lock(&mut self) {
        assert!(!self.locked, "the monitoring mutex is not recursive here");
        self.locked = true;
        self.steps.push(Step::Lock);
    }
    fn unlock(&mut self) {
        assert!(self.locked);
        self.locked = false;
        self.steps.push(Step::Unlock);
    }
    fn count(&mut self) -> usize {
        assert!(self.locked, "the monitor list is read under the lock");
        self.steps.push(Step::Count);
        self.monitors.len()
    }
    fn reset(&mut self, index: usize) {
        assert!(self.locked, "monitors are reset under the lock");
        self.steps.push(Step::Reset(index));
        let id = self.id.clone();
        let monitor = &mut self.monitors[index];
        monitor.0 = id;
        monitor.1 += 1;
    }
}

impl Device for Model {
    type Str = &'static str;
    type Source = usize;
    fn empty(&mut self, s: &'static str) -> bool {
        self.steps.push(Step::Empty(s.into()));
        s.is_empty()
    }
    fn compare(&mut self, id: &'static str) -> i32 {
        assert!(self.locked, "the stored id is compared under the lock");
        self.steps.push(Step::Compare(id.into()));
        match id.cmp(self.id.as_str()) {
            core::cmp::Ordering::Less => -1,
            core::cmp::Ordering::Equal => 0,
            core::cmp::Ordering::Greater => 1,
        }
    }
    fn store(&mut self, name: &'static str, id: &'static str) {
        assert!(self.locked, "the copies are stored under the lock");
        self.steps.push(Step::Store(name.into(), id.into()));
        self.name = name.into();
        self.id = id.into();
    }
    fn enum_sources(&mut self) {
        assert!(!self.locked, "sources are enumerated after the unlock");
        self.steps.push(Step::Enum);
        let public: Vec<usize> = (0..self.sources.len())
            .filter(|&i| self.sources[i].public)
            .collect();
        for index in public {
            if !aoc_source(self, index) {
                break;
            }
        }
    }
    fn output_flags(&mut self, source: usize) -> u32 {
        self.steps.push(Step::Flags(source));
        self.sources[source].flags
    }
    fn notify(&mut self, source: usize) {
        let device_id = self.sources[source].device_id;
        self.steps.push(Step::Notify(source, device_id.into()));
        if device_id == format!("{}.monitor", self.id) {
            self.signals.push(Some(source));
            self.duplicating = Some(source);
        } else if self.duplicating == Some(source) {
            self.signals.push(None);
            self.duplicating = None;
        }
    }
}

fn s(text: &str) -> String {
    text.into()
}

fn leak(text: &str) -> &'static str {
    Box::leak(text.to_owned().into_boxed_str())
}

#[test]
fn reset_without_monitors_is_noop() {
    let mut model = Model::new();
    assert!(model.set(Some("Sink A"), Some("sink_a")));
    model.take_steps();
    reset_monitoring(&mut model);
    reset_monitoring(&mut model);
    let once = [Step::Available, Step::Lock, Step::Count, Step::Unlock];
    assert_eq!(model.take_steps(), [once.clone(), once].concat());
    assert_eq!(model.device(), ("Sink A", "sink_a"));
}

#[test]
fn reset_resets_each_live_monitor_once() {
    let mut model = Model::with_monitors(2);
    assert!(model.set(Some("Sink A"), Some("sink_a")));
    assert_eq!(model.resets(), [1, 1]);
    reset_monitoring(&mut model);
    assert_eq!(model.resets(), [2, 2]);
    reset_monitoring(&mut model);
    assert_eq!(model.resets(), [3, 3]);
    assert!(model.monitors.iter().all(|(device, _)| device == "sink_a"));
    assert_eq!(model.device(), ("Sink A", "sink_a"));
}

#[test]
fn new_device_is_copied_and_resets_monitors() {
    let mut model = Model::with_monitors(2);
    assert!(model.set(Some("Sink A"), Some("sink_a")));
    let mut name = String::from("Sink B");
    let mut id = String::from("sink_b");
    assert!(model.set(Some(&name), Some(&id)));
    assert_eq!(model.resets(), [2, 2]);
    assert!(model.monitors.iter().all(|(device, _)| device == "sink_b"));
    // The core keeps its own copies, not the caller's storage.
    name.replace_range(.., "Overwritten");
    id.replace_range(.., "overwritten");
    assert_eq!(model.device(), ("Sink B", "sink_b"));
    // A later reset uses the stored id.
    reset_monitoring(&mut model);
    assert_eq!(model.resets(), [3, 3]);
    assert!(model.monitors.iter().all(|(device, _)| device == "sink_b"));
}

#[test]
fn same_device_keeps_name_and_resets_nothing() {
    let mut model = Model::with_monitors(1);
    model.sources.push(Source {
        flags: AUDIO | DO_NOT_SELF_MONITOR,
        device_id: "sink_a.monitor",
        public: true,
    });
    assert!(model.set(Some("Sink A"), Some("sink_a")));
    let resets = model.resets();
    model.signals.clear();
    model.take_steps();
    assert!(model.set(Some("Renamed"), Some("sink_a")));
    assert_eq!(model.device(), ("Sink A", "sink_a"));
    assert_eq!(model.resets(), resets);
    assert!(model.signals.is_empty());
    assert_eq!(
        model.take_steps(),
        [
            Step::Empty(s("Renamed")),
            Step::Empty(s("sink_a")),
            Step::Available,
            Step::Lock,
            Step::Compare(s("sink_a")),
            Step::Unlock,
        ]
    );
}

#[test]
fn aoc_sources_told_with_their_device_id() {
    let mut model = Model::new();
    assert!(model.set(Some("Sink A"), Some("sink_a")));
    let aoc = AUDIO | DO_NOT_SELF_MONITOR;
    for (flags, device_id, public) in [
        (aoc, "sink_b.monitor", true),
        (aoc, "sink_c.monitor", true),
        (AUDIO, "sink_b.monitor", true),
        (aoc, "sink_b.monitor", false),
    ] {
        model.sources.push(Source {
            flags,
            device_id,
            public,
        });
    }
    let (aoc_b, aoc_c) = (0, 1);

    // Only the public capture source of the new device starts it.
    assert!(model.set(Some("Sink B"), Some("sink_b")));
    assert_eq!(model.signals, [Some(aoc_b)]);

    // aoc b no longer matches and was deduplicating: it stops it; then aoc
    // c, which matches, starts it.
    model.signals.clear();
    assert!(model.set(Some("Sink C"), Some("sink_c")));
    assert_eq!(model.signals, [None, Some(aoc_c)]);

    // No capture source matches: only the deduplicating one is stopped.
    model.signals.clear();
    assert!(model.set(Some("Sink D"), Some("sink_d")));
    assert_eq!(model.signals, [None]);

    // Neither is deduplicating any more: a new id tells nobody.
    model.signals.clear();
    model.take_steps();
    assert!(model.set(Some("Sink A"), Some("sink_a")));
    assert!(model.signals.is_empty());

    // Every public source is asked for its flags; only the two capture
    // sources are told, each with its own device_id, in enumeration order.
    let steps = model.take_steps();
    let tail = steps
        .iter()
        .skip_while(|step| **step != Step::Enum)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        tail,
        [
            Step::Enum,
            Step::Flags(0),
            Step::Notify(0, s("sink_b.monitor")),
            Step::Flags(1),
            Step::Notify(1, s("sink_c.monitor")),
            Step::Flags(2),
        ]
    );
}

#[test]
fn missing_or_empty_name_or_id_refused_before_anything() {
    let cases: [(Option<&str>, Option<&str>, Vec<Step>); 6] = [
        (None, Some("sink"), vec![]),
        (Some("Sink"), None, vec![]),
        (None, None, vec![]),
        (Some(""), Some("sink"), vec![Step::Empty(s(""))]),
        (
            Some("Sink"),
            Some(""),
            vec![Step::Empty(s("Sink")), Step::Empty(s(""))],
        ),
        (Some(""), Some(""), vec![Step::Empty(s(""))]),
    ];
    for (name, id, steps) in cases {
        let mut model = Model::with_monitors(1);
        assert!(!model.set(name, id), "{name:?} {id:?}");
        assert_eq!(model.take_steps(), steps, "{name:?} {id:?}");
        assert_eq!(model.device(), ("Default", "default"));
        assert_eq!(model.resets(), [0]);
    }
}

#[test]
fn unavailable_monitoring_refused_after_the_checks() {
    let mut model = Model::with_monitors(1);
    model.available = false;
    assert!(!model.set(Some("Sink"), Some("sink")));
    assert_eq!(
        model.take_steps(),
        [
            Step::Empty(s("Sink")),
            Step::Empty(s("sink")),
            Step::Available
        ]
    );
    assert_eq!(model.device(), ("Default", "default"));

    reset_monitoring(&mut model);
    assert_eq!(model.take_steps(), [Step::Available]);
    assert_eq!(model.resets(), [0]);
}

#[test]
fn new_device_runs_every_step_in_order() {
    let mut model = Model::with_monitors(2);
    model.sources.push(Source {
        flags: AUDIO | DO_NOT_SELF_MONITOR,
        device_id: "x",
        public: true,
    });
    model.sources.push(Source {
        flags: AUDIO,
        device_id: "y",
        public: true,
    });
    assert!(model.set(Some("Name"), Some("id")));
    assert_eq!(
        model.take_steps(),
        [
            Step::Empty(s("Name")),
            Step::Empty(s("id")),
            Step::Available,
            Step::Lock,
            Step::Compare(s("id")),
            Step::Store(s("Name"), s("id")),
            Step::Unlock,
            Step::Available,
            Step::Lock,
            Step::Count,
            Step::Reset(0),
            Step::Count,
            Step::Reset(1),
            Step::Count,
            Step::Unlock,
            Step::Enum,
            Step::Flags(0),
            Step::Notify(0, s("x")),
            Step::Flags(1),
        ]
    );
}

#[test]
fn monitoring_lost_between_store_and_reset_skips_the_resets() {
    // Availability is asked again by the reset, as the C calls
    // obs_reset_audio_monitoring(): the store stands, no monitor is reset,
    // and the sources are still told.
    struct Flaky(Model, usize);
    impl Monitors for Flaky {
        fn available(&mut self) -> bool {
            self.1 += 1;
            self.0.steps.push(Step::Available);
            self.1 == 1
        }
        fn lock(&mut self) {
            self.0.lock();
        }
        fn unlock(&mut self) {
            self.0.unlock();
        }
        fn count(&mut self) -> usize {
            self.0.count()
        }
        fn reset(&mut self, index: usize) {
            self.0.reset(index);
        }
    }
    impl Device for Flaky {
        type Str = &'static str;
        type Source = usize;
        fn empty(&mut self, s: &'static str) -> bool {
            self.0.empty(s)
        }
        fn compare(&mut self, id: &'static str) -> i32 {
            self.0.compare(id)
        }
        fn store(&mut self, name: &'static str, id: &'static str) {
            self.0.store(name, id);
        }
        fn enum_sources(&mut self) {
            self.0.enum_sources();
        }
        fn output_flags(&mut self, source: usize) -> u32 {
            self.0.output_flags(source)
        }
        fn notify(&mut self, source: usize) {
            self.0.notify(source);
        }
    }
    let mut flaky = Flaky(Model::with_monitors(1), 0);
    assert!(set_device(&mut flaky, Some("N"), Some("i")));
    assert_eq!(flaky.0.device(), ("N", "i"));
    assert_eq!(flaky.0.resets(), [0]);
    assert_eq!(flaky.1, 2);
    assert_eq!(flaky.0.steps.last(), Some(&Step::Enum));
}

#[test]
fn aoc_source_always_continues() {
    let mut model = Model::new();
    model.sources.push(Source {
        flags: 0,
        device_id: "a",
        public: true,
    });
    model.sources.push(Source {
        flags: DO_NOT_SELF_MONITOR,
        device_id: "b",
        public: true,
    });
    assert!(aoc_source(&mut model, 0));
    assert!(aoc_source(&mut model, 1));
    assert_eq!(
        model.take_steps(),
        [Step::Flags(0), Step::Flags(1), Step::Notify(1, s("b"))]
    );
}
