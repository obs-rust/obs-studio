//! Safe mirrors of test_source_push_to_enable.c, plus lock/log sequencing.
use obs_source::push_to_enable::{Kind, Source, enable};
#[derive(Default)]
struct Model {
    values: [bool; 2],
    calls: [usize; 2],
    flags: u32,
    locked: bool,
    events: Vec<&'static str>,
}
fn index(kind: Kind) -> usize {
    usize::from(kind == Kind::Talk)
}
impl Source for Model {
    fn lock(&mut self) {
        assert!(!self.locked);
        self.locked = true;
        self.events.push("lock");
    }
    fn unlock(&mut self) {
        assert!(self.locked);
        self.events.push("unlock");
        self.locked = false;
    }
    fn enabled(&self, kind: Kind) -> bool {
        assert!(self.locked);
        self.values[index(kind)]
    }
    fn output_flags(&self) -> u32 {
        assert!(self.locked);
        self.flags
    }
    fn log_change(&mut self, _: Kind, _: bool) {
        assert!(self.locked);
        self.events.push("log");
    }
    fn store(&mut self, kind: Kind, enabled: bool) {
        assert!(self.locked);
        self.values[index(kind)] = enabled;
        self.events.push("store");
    }
    fn signal(&mut self, kind: Kind, enabled: bool) {
        assert!(self.locked);
        // Mirrors source identity/enable payload and state visibility in changed().
        assert_eq!(self.values[index(kind)], enabled);
        self.calls[index(kind)] += 1;
        self.events.push("signal");
    }
}
#[test]
fn test_source_push_to_enable_transitions() {
    let mut source = Model::default();
    let mut previous = false;
    let mut expected = 0;
    for value in [false, true, true, false, false, true] {
        expected += usize::from(value != previous);
        enable(Some(&mut source), Kind::Mute, value, |_| {
            panic!("valid source")
        });
        assert_eq!(
            source.calls,
            [expected, expected - usize::from(value != previous)]
        );
        assert_eq!(source.values, [value, previous]);
        assert!(!source.locked);
        enable(Some(&mut source), Kind::Talk, value, |_| {
            panic!("valid source")
        });
        assert_eq!(source.calls, [expected; 2]);
        assert_eq!(source.values, [value; 2]);
        assert!(!source.locked);
        previous = value;
    }
}
#[test]
fn test_source_push_to_enable_null_source() {
    let mut messages = Vec::new();
    for enabled in [false, true] {
        for kind in [Kind::Mute, Kind::Talk] {
            enable::<Model>(None, kind, enabled, |name| {
                messages.push(format!("{name}: Null 'source' parameter"));
            });
        }
    }
    assert_eq!(
        messages,
        [
            "obs_source_enable_push_to_mute: Null 'source' parameter",
            "obs_source_enable_push_to_talk: Null 'source' parameter",
            "obs_source_enable_push_to_mute: Null 'source' parameter",
            "obs_source_enable_push_to_talk: Null 'source' parameter",
        ]
    );
}
#[test]
fn logs_only_audio_changes_before_store_and_signal() {
    for kind in [Kind::Mute, Kind::Talk] {
        for flags in [0, 1, 2, 3, u32::MAX] {
            let mut source = Model {
                flags,
                ..Model::default()
            };
            enable(Some(&mut source), kind, true, |_| panic!("valid source"));
            let expected = if flags & 2 != 0 {
                vec!["lock", "log", "store", "signal", "unlock"]
            } else {
                vec!["lock", "store", "signal", "unlock"]
            };
            assert_eq!(source.events, expected);
            source.events.clear();
            enable(Some(&mut source), kind, true, |_| panic!("valid source"));
            assert_eq!(source.events, ["lock", "store", "unlock"]);
        }
    }
}
