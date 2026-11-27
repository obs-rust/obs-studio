//! Tier 1 mirrors of test_source_output_capture_device.c, plus what the C
//! test can't reach: no monitoring device (the core always holds one once
//! started) and the host's default device (on Linux the hook always
//! declines). The model records every step the check takes, in order.
use obs_source::output_capture_device::{
    DO_NOT_SELF_MONITOR, Device, device_activated, device_changed,
};

const A: usize = 0;
const B: usize = 1;
const NODES: usize = 2;

type Id = &'static str;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Step {
    MonitoringSet,
    Flags(usize),
    MonitoringId,
    HostDefault(Option<Id>),
    Match(Option<Id>, Option<Id>),
    FreeDefault(Id),
    Signal(Option<usize>),
    Connect(usize),
    Disconnect(usize),
    LogStarted(usize),
    LogStopped,
    Duplicating,
    CalldataSource,
    Settings(usize),
    SettingsId(usize),
    ReleaseSettings(usize),
}

struct Model {
    monitoring_set: bool,
    flags: [u32; NODES],
    monitoring_id: Option<Id>,
    host: Option<Option<Id>>,
    duplicating: Option<usize>,
    settings_ids: [Option<Id>; NODES],
    steps: Vec<Step>,
}

impl Model {
    /// Both sources capture output devices; "sink_a" is monitored; the host
    /// declines; nobody deduplicates.
    fn new() -> Self {
        Self {
            monitoring_set: true,
            flags: [DO_NOT_SELF_MONITOR; NODES],
            monitoring_id: Some("sink_a"),
            host: None,
            duplicating: None,
            settings_ids: [Some("sink_a"), Some("sink_b")],
            steps: Vec::new(),
        }
    }
}

impl Device for Model {
    type Source = usize;
    type Id = Id;
    type Calldata = Option<usize>;
    type Settings = usize;

    fn monitoring_device_set(&mut self) -> bool {
        self.steps.push(Step::MonitoringSet);
        self.monitoring_set
    }
    fn output_flags(&mut self, source: usize) -> u32 {
        self.steps.push(Step::Flags(source));
        self.flags[source]
    }
    fn monitoring_id(&mut self) -> Option<Id> {
        self.steps.push(Step::MonitoringId);
        self.monitoring_id
    }
    fn host_default(&mut self, device_id: Option<Id>) -> Option<Option<Id>> {
        self.steps.push(Step::HostDefault(device_id));
        self.host
    }
    fn devices_match(&mut self, id1: Option<Id>, id2: Option<Id>) -> bool {
        self.steps.push(Step::Match(id1, id2));
        id1.is_some() && id1 == id2
    }
    fn free_default(&mut self, id: Id) {
        self.steps.push(Step::FreeDefault(id));
    }
    fn signal_deduplication(&mut self, source: Option<usize>) {
        self.steps.push(Step::Signal(source));
    }
    fn connect_activate(&mut self, source: usize) {
        self.steps.push(Step::Connect(source));
    }
    fn disconnect_activate(&mut self, source: usize) {
        self.steps.push(Step::Disconnect(source));
    }
    fn log_started(&mut self, source: usize) {
        self.steps.push(Step::LogStarted(source));
    }
    fn log_stopped(&mut self) {
        self.steps.push(Step::LogStopped);
    }
    fn duplicating_source(&mut self) -> Option<usize> {
        self.steps.push(Step::Duplicating);
        self.duplicating
    }
    fn calldata_source(&mut self, cd: Option<usize>) -> Option<usize> {
        self.steps.push(Step::CalldataSource);
        cd
    }
    fn settings(&mut self, source: usize) -> usize {
        self.steps.push(Step::Settings(source));
        source
    }
    fn settings_device_id(&mut self, settings: usize) -> Option<Id> {
        self.steps.push(Step::SettingsId(settings));
        self.settings_ids[settings]
    }
    fn release_settings(&mut self, settings: usize) {
        self.steps.push(Step::ReleaseSettings(settings));
    }
}

/// The reads up to the match, with the host declining.
fn checked(source: usize, device_id: Option<Id>, monitored: Option<Id>) -> Vec<Step> {
    vec![
        Step::MonitoringSet,
        Step::Flags(source),
        Step::MonitoringId,
        Step::HostDefault(device_id),
        Step::Match(device_id, monitored),
    ]
}

fn started(source: usize) -> [Step; 3] {
    [
        Step::Signal(Some(source)),
        Step::Connect(source),
        Step::LogStarted(source),
    ]
}

fn stopped(source: usize) -> [Step; 4] {
    [
        Step::Duplicating,
        Step::Disconnect(source),
        Step::Signal(None),
        Step::LogStopped,
    ]
}

#[test]
fn no_monitoring_device_reads_nothing_else() {
    let mut m = Model::new();
    m.monitoring_set = false;
    m.duplicating = Some(A);
    device_changed(&mut m, A, Some("sink_a"));
    assert_eq!(m.steps, [Step::MonitoringSet]);
}

#[test]
fn source_without_flag_is_ignored() {
    for flags in [0, !DO_NOT_SELF_MONITOR, DO_NOT_SELF_MONITOR >> 1] {
        let mut m = Model::new();
        m.flags[A] = flags;
        m.duplicating = Some(A);
        device_changed(&mut m, A, Some("sink_a"));
        assert_eq!(m.steps, [Step::MonitoringSet, Step::Flags(A)], "{flags:#x}");
    }
}

#[test]
fn any_flags_with_the_bit_are_checked() {
    for flags in [DO_NOT_SELF_MONITOR, u32::MAX] {
        let mut m = Model::new();
        m.flags[A] = flags;
        device_changed(&mut m, A, Some("sink_a"));
        let mut want = checked(A, Some("sink_a"), Some("sink_a"));
        want.extend(started(A));
        assert_eq!(m.steps, want, "{flags:#x}");
    }
}

#[test]
fn matching_device_starts_deduplication() {
    // Whoever deduplicates now, a match takes over without a stop, and the
    // duplicating source is never read.
    for duplicating in [None, Some(A), Some(B)] {
        let mut m = Model::new();
        m.duplicating = duplicating;
        device_changed(&mut m, A, Some("sink_a"));
        let mut want = checked(A, Some("sink_a"), Some("sink_a"));
        want.extend(started(A));
        assert_eq!(m.steps, want, "{duplicating:?}");
    }
}

#[test]
fn non_matching_stops_only_the_deduplicating_source() {
    for device_id in [Some("sink_b"), None] {
        let mut m = Model::new();
        m.duplicating = Some(A);
        device_changed(&mut m, A, device_id);
        let mut want = checked(A, device_id, Some("sink_a"));
        want.extend(stopped(A));
        assert_eq!(m.steps, want, "{device_id:?}");

        for duplicating in [None, Some(B)] {
            let mut m = Model::new();
            m.duplicating = duplicating;
            device_changed(&mut m, A, device_id);
            let mut want = checked(A, device_id, Some("sink_a"));
            want.push(Step::Duplicating);
            assert_eq!(m.steps, want, "{device_id:?} {duplicating:?}");
        }
    }
}

#[test]
fn no_monitoring_id_is_passed_on() {
    let mut m = Model::new();
    m.monitoring_id = None;
    m.duplicating = Some(A);
    device_changed(&mut m, A, Some("sink_a"));
    let mut want = checked(A, Some("sink_a"), None);
    want.extend(stopped(A));
    assert_eq!(m.steps, want);
}

#[test]
fn host_default_is_matched_and_freed() {
    // The host's id replaces the source's, is matched, then freed before
    // the outcome.
    let mut m = Model::new();
    m.host = Some(Some("sink_a"));
    device_changed(&mut m, A, Some("default"));
    let mut want = vec![
        Step::MonitoringSet,
        Step::Flags(A),
        Step::MonitoringId,
        Step::HostDefault(Some("default")),
        Step::Match(Some("sink_a"), Some("sink_a")),
        Step::FreeDefault("sink_a"),
    ];
    want.extend(started(A));
    assert_eq!(m.steps, want);

    let mut m = Model::new();
    m.host = Some(Some("sink_b"));
    m.duplicating = Some(A);
    device_changed(&mut m, A, Some("sink_a"));
    let mut want = vec![
        Step::MonitoringSet,
        Step::Flags(A),
        Step::MonitoringId,
        Step::HostDefault(Some("sink_a")),
        Step::Match(Some("sink_b"), Some("sink_a")),
        Step::FreeDefault("sink_b"),
    ];
    want.extend(stopped(A));
    assert_eq!(m.steps, want);
}

#[test]
fn host_default_null_is_matched_and_not_freed() {
    let mut m = Model::new();
    m.host = Some(None);
    m.duplicating = Some(A);
    device_changed(&mut m, A, Some("sink_a"));
    let mut want = vec![
        Step::MonitoringSet,
        Step::Flags(A),
        Step::MonitoringId,
        Step::HostDefault(Some("sink_a")),
        Step::Match(None, Some("sink_a")),
    ];
    want.extend(stopped(A));
    assert_eq!(m.steps, want);
}

#[test]
fn activate_without_source_does_nothing() {
    let mut m = Model::new();
    device_activated(&mut m, None);
    assert_eq!(m.steps, [Step::CalldataSource]);
}

#[test]
fn activate_checks_the_settings_device() {
    // A's settings match: started, then the settings released.
    let mut m = Model::new();
    device_activated(&mut m, Some(A));
    let mut want = vec![Step::CalldataSource, Step::Settings(A), Step::SettingsId(A)];
    want.extend(checked(A, Some("sink_a"), Some("sink_a")));
    want.extend(started(A));
    want.push(Step::ReleaseSettings(A));
    assert_eq!(m.steps, want);

    // B deduplicates and its settings don't match: stopped.
    let mut m = Model::new();
    m.duplicating = Some(B);
    device_activated(&mut m, Some(B));
    let mut want = vec![Step::CalldataSource, Step::Settings(B), Step::SettingsId(B)];
    want.extend(checked(B, Some("sink_b"), Some("sink_a")));
    want.extend(stopped(B));
    want.push(Step::ReleaseSettings(B));
    assert_eq!(m.steps, want);

    // No monitoring device: the settings are still read and released.
    let mut m = Model::new();
    m.monitoring_set = false;
    device_activated(&mut m, Some(A));
    assert_eq!(
        m.steps,
        [
            Step::CalldataSource,
            Step::Settings(A),
            Step::SettingsId(A),
            Step::MonitoringSet,
            Step::ReleaseSettings(A),
        ]
    );
}
