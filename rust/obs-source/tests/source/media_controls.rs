//! Tier 1 mirrors of test_source_media_controls.c without FFI or unsafe
//! code, on a model source that records every access in order.
use core::ffi::CStr;
use obs_source::media_admission::{CONTROLLABLE_MEDIA, Call};
use obs_source::media_controls::{
    Action, ENDED_FUNCTION, ENDED_SIGNAL, NEXT_FUNCTION, PLAY_PAUSE_FUNCTION, PREVIOUS_FUNCTION,
    RESTART_FUNCTION, SET_TIME_FUNCTION, STARTED_FUNCTION, STARTED_SIGNAL, STOP_FUNCTION, Source,
    ended, queue, started,
};
use std::cell::RefCell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    HasData,
    OutputFlags,
    HasCallback(Call),
    Queue(Action),
    Signal(&'static CStr),
}

/// A model source: `callbacks` holds the calls whose callback is set.
struct Model {
    data: bool,
    output_flags: u32,
    callbacks: Vec<Call>,
    events: RefCell<Vec<Event>>,
}

impl Model {
    fn new(data: bool, output_flags: u32, callbacks: &[Call]) -> Self {
        Self {
            data,
            output_flags,
            callbacks: callbacks.to_vec(),
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
    fn has_data(&self) -> bool {
        self.push(Event::HasData);
        self.data
    }
    fn output_flags(&self) -> u32 {
        self.push(Event::OutputFlags);
        self.output_flags
    }
    fn has_callback(&self, call: Call) -> bool {
        self.push(Event::HasCallback(call));
        self.callbacks.contains(&call)
    }
    fn queue(&self, action: Action) {
        self.push(Event::Queue(action));
    }
    fn signal(&self, signal: &'static CStr) {
        self.push(Event::Signal(signal));
    }
}

fn no_log(function: &'static CStr) {
    panic!("logged {function:?}");
}

const QUEUEING: [Call; 6] = [
    Call::PlayPause,
    Call::Restart,
    Call::Stop,
    Call::Next,
    Call::Previous,
    Call::SetTime,
];

const FLAG_WORDS: [u32; 6] = [
    0,
    CONTROLLABLE_MEDIA,
    CONTROLLABLE_MEDIA | 1 | 2,
    !CONTROLLABLE_MEDIA,
    u32::MAX,
    1 | 4,
];

fn actions(pause: bool, ms: i64) -> [Action; 6] {
    [
        Action::PlayPause(pause),
        Action::Restart,
        Action::Stop,
        Action::Next,
        Action::Previous,
        Action::SetTime(ms),
    ]
}

#[test]
fn null_source_logs_once() {
    let mut logs = Vec::new();
    for action in actions(true, 1) {
        queue(None::<&Model>, action, |f| logs.push(f));
    }
    started(None::<&Model>, |f| logs.push(f));
    ended(None::<&Model>, |f| logs.push(f));
    assert_eq!(
        logs,
        [
            PLAY_PAUSE_FUNCTION,
            RESTART_FUNCTION,
            STOP_FUNCTION,
            NEXT_FUNCTION,
            PREVIOUS_FUNCTION,
            SET_TIME_FUNCTION,
            STARTED_FUNCTION,
            ENDED_FUNCTION
        ]
    );
    assert_eq!(PLAY_PAUSE_FUNCTION, c"obs_source_media_play_pause");
    assert_eq!(RESTART_FUNCTION, c"obs_source_media_restart");
    assert_eq!(STOP_FUNCTION, c"obs_source_media_stop");
    assert_eq!(NEXT_FUNCTION, c"obs_source_media_next");
    assert_eq!(PREVIOUS_FUNCTION, c"obs_source_media_previous");
    assert_eq!(SET_TIME_FUNCTION, c"obs_source_media_set_time");
    assert_eq!(STARTED_FUNCTION, c"obs_source_media_started");
    assert_eq!(ENDED_FUNCTION, c"obs_source_media_ended");
}

#[test]
fn queueing_calls_need_data_flags_and_their_callback() {
    for action in actions(true, 7) {
        let call = action.call();
        assert!(QUEUEING.contains(&call));
        for output_flags in FLAG_WORDS {
            for data in [false, true] {
                for own in [false, true] {
                    for others in [false, true] {
                        let mut callbacks: Vec<Call> = if others {
                            QUEUEING.iter().copied().filter(|c| *c != call).collect()
                        } else {
                            Vec::new()
                        };
                        if own {
                            callbacks.push(call);
                        }
                        let model = Model::new(data, output_flags, &callbacks);
                        queue(Some(&model), action, no_log);
                        // Each value is read only when the step before passed.
                        let mut expected = vec![Event::HasData];
                        if data {
                            expected.push(Event::OutputFlags);
                            if output_flags & CONTROLLABLE_MEDIA != 0 {
                                expected.push(Event::HasCallback(call));
                                if own {
                                    expected.push(Event::Queue(action));
                                }
                            }
                        }
                        assert_eq!(model.take(), expected, "{action:?} {output_flags:#x}");
                    }
                }
            }
        }
    }
}

#[test]
fn accepted_calls_queue_their_action() {
    let model = Model::new(true, CONTROLLABLE_MEDIA, &QUEUEING);
    for (pause, ms) in [
        (false, 0),
        (true, 1),
        (false, -1),
        (true, i64::MIN),
        (false, i64::MAX),
        (true, 0x1_2345_6789),
    ] {
        for action in actions(pause, ms) {
            queue(Some(&model), action, no_log);
            assert_eq!(
                model.take(),
                [
                    Event::HasData,
                    Event::OutputFlags,
                    Event::HasCallback(action.call()),
                    Event::Queue(action)
                ]
            );
        }
    }
    assert_eq!(Action::PlayPause(true).action_type(), 1);
    assert_eq!(Action::Restart.action_type(), 2);
    assert_eq!(Action::Stop.action_type(), 3);
    assert_eq!(Action::Next.action_type(), 4);
    assert_eq!(Action::Previous.action_type(), 5);
    assert_eq!(Action::SetTime(9).action_type(), 6);
}

#[test]
fn started_and_ended_signal_any_controllable_source() {
    type Signaller = fn(Option<&Model>, fn(&'static CStr));
    let calls: [(Signaller, &CStr); 2] = [
        (|s, l| started(s, l), STARTED_SIGNAL),
        (|s, l| ended(s, l), ENDED_SIGNAL),
    ];
    assert_eq!(STARTED_SIGNAL, c"media_started");
    assert_eq!(ENDED_SIGNAL, c"media_ended");
    for (signaller, signal) in calls {
        for output_flags in FLAG_WORDS {
            for data in [false, true] {
                for callbacks in [&[][..], &QUEUEING[..]] {
                    let model = Model::new(data, output_flags, callbacks);
                    signaller(Some(&model), no_log);
                    // No context.data or callback read, and nothing queued.
                    let mut expected = vec![Event::OutputFlags];
                    if output_flags & CONTROLLABLE_MEDIA != 0 {
                        expected.push(Event::Signal(signal));
                    }
                    assert_eq!(model.take(), expected);
                }
            }
        }
    }
}
