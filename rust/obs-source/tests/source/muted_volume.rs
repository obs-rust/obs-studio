//! Tier 1: `obs_source::muted_volume` over a recording model of the source's
//! fields, mirroring the `test_source_muted_volume` cases one field at a
//! time: the stop-time writes before the reads, each step's own flags (the
//! user's for `source_muted()`, the mix thread's for `get_source_volume()`),
//! `enabled` before the mute flag before each key's gate, each read only
//! where the C's `||` reaches it, and `volume` read once, muted or not.
use obs_source::muted_volume::{Flags, Host, Key, get_source_volume, source_muted};
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Event {
    KeyEnabled(Key),
    Pressed(Flags, Key),
    Delay(Key),
    StopTime(Key),
    SetStopTime(Key, u64),
    Enabled,
    Muted(Flags),
    Volume,
}

const MS: u64 = 1_000_000;
const T: u64 = 1_000_000 * MS;

#[derive(Debug, Clone, Default)]
struct State {
    enabled: bool,
    user_muted: bool,
    muted: bool,
    /// Indexed by [`idx`].
    key_enabled: [bool; 2],
    user_pressed: [bool; 2],
    pressed: [bool; 2],
    delay: [u64; 2],
    stop: [u64; 2],
    volume: f32,
}

const fn idx(key: Key) -> usize {
    match key {
        Key::Mute => 0,
        Key::Talk => 1,
    }
}

struct Model {
    state: RefCell<State>,
    events: RefCell<Vec<Event>>,
}

impl Model {
    fn new(state: State) -> Self {
        Model {
            state: RefCell::new(state),
            events: RefCell::new(Vec::new()),
        }
    }
    fn push(&self, event: Event) {
        self.events.borrow_mut().push(event);
    }
    fn events(&self) -> Vec<Event> {
        std::mem::take(&mut self.events.borrow_mut())
    }
    fn stop(&self, key: Key) -> u64 {
        self.state.borrow().stop[idx(key)]
    }
}

impl Host for Model {
    type Source = ();
    fn key_enabled(&self, (): (), key: Key) -> bool {
        self.push(Event::KeyEnabled(key));
        self.state.borrow().key_enabled[idx(key)]
    }
    fn pressed(&self, (): (), flags: Flags, key: Key) -> bool {
        self.push(Event::Pressed(flags, key));
        let s = self.state.borrow();
        match flags {
            Flags::User => s.user_pressed[idx(key)],
            Flags::Mix => s.pressed[idx(key)],
        }
    }
    fn delay(&self, (): (), key: Key) -> u64 {
        self.push(Event::Delay(key));
        self.state.borrow().delay[idx(key)]
    }
    fn stop_time(&self, (): (), key: Key) -> u64 {
        self.push(Event::StopTime(key));
        self.stop(key)
    }
    fn set_stop_time(&self, (): (), key: Key, stop_time: u64) {
        self.push(Event::SetStopTime(key, stop_time));
        self.state.borrow_mut().stop[idx(key)] = stop_time;
    }
    fn enabled(&self, (): ()) -> bool {
        self.push(Event::Enabled);
        self.state.borrow().enabled
    }
    fn muted(&self, (): (), flags: Flags) -> bool {
        self.push(Event::Muted(flags));
        let s = self.state.borrow();
        match flags {
            Flags::User => s.user_muted,
            Flags::Mix => s.muted,
        }
    }
    fn volume(&self, (): ()) -> f32 {
        self.push(Event::Volume);
        self.state.borrow().volume
    }
}

fn base() -> State {
    State {
        enabled: true,
        volume: 0.5,
        ..State::default()
    }
}

/// The reads of a step whose keys are neither enabled nor pressed and whose
/// stop times are in the past, up to the gate's result.
fn idle_reads(flags: Flags) -> Vec<Event> {
    vec![
        Event::KeyEnabled(Key::Mute),
        Event::KeyEnabled(Key::Talk),
        Event::Pressed(flags, Key::Mute),
        Event::StopTime(Key::Mute),
        Event::Pressed(flags, Key::Talk),
        Event::StopTime(Key::Talk),
        Event::Enabled,
        Event::Muted(flags),
        Event::KeyEnabled(Key::Mute),
        Event::KeyEnabled(Key::Talk),
    ]
}

#[test]
fn idle_source_reads_every_field_once_in_order() {
    let model = Model::new(base());
    assert!(!source_muted(&model, (), T));
    assert_eq!(model.events(), idle_reads(Flags::User));

    let model = Model::new(base());
    assert_eq!(get_source_volume(&model, (), T).to_bits(), 0.5f32.to_bits());
    let mut reads = idle_reads(Flags::Mix);
    reads.push(Event::Volume);
    assert_eq!(model.events(), reads);
}

#[test]
fn disabled_source_short_circuits_before_the_mute_flag() {
    let model = Model::new(State {
        enabled: false,
        user_muted: true,
        ..base()
    });
    assert!(source_muted(&model, (), T));
    let events = model.events();
    assert_eq!(events.last(), Some(&Event::Enabled));
    assert!(!events.contains(&Event::Muted(Flags::User)));

    let model = Model::new(State {
        enabled: false,
        ..base()
    });
    assert_eq!(get_source_volume(&model, (), T).to_bits(), 0.0f32.to_bits());
    let events = model.events();
    assert_eq!(&events[events.len() - 2..], [Event::Enabled, Event::Volume]);
    assert!(!events.contains(&Event::Muted(Flags::Mix)));
}

#[test]
fn mute_flag_short_circuits_before_the_keys() {
    for flags in [Flags::User, Flags::Mix] {
        let mut state = base();
        match flags {
            Flags::User => state.user_muted = true,
            Flags::Mix => state.muted = true,
        }
        let model = Model::new(state);
        let muted = match flags {
            Flags::User => source_muted(&model, (), T),
            Flags::Mix => get_source_volume(&model, (), T) == 0.0,
        };
        assert!(muted);
        let events = model.events();
        let after = events
            .iter()
            .position(|e| *e == Event::Muted(flags))
            .unwrap();
        let tail: Vec<_> = events[after + 1..]
            .iter()
            .filter(|e| **e != Event::Volume)
            .collect();
        assert!(tail.is_empty(), "{flags:?} {tail:?}");
    }
}

#[test]
fn each_step_reads_only_its_own_flags() {
    // The other step's flags all set: neither mutes.
    let model = Model::new(State {
        muted: true,
        key_enabled: [true, false],
        pressed: [true, false],
        ..base()
    });
    assert!(!source_muted(&model, (), T));
    assert!(
        model
            .events()
            .iter()
            .all(|e| !matches!(e, Event::Pressed(Flags::Mix, _) | Event::Muted(Flags::Mix)))
    );
    // Nor did the user step write the mute key's stop time.
    assert_eq!(model.stop(Key::Mute), 0);

    let model = Model::new(State {
        user_muted: true,
        key_enabled: [true, false],
        user_pressed: [true, false],
        ..base()
    });
    assert_eq!(get_source_volume(&model, (), T).to_bits(), 0.5f32.to_bits());
    assert!(model.events().iter().all(|e| !matches!(
        e,
        Event::Pressed(Flags::User, _) | Event::Muted(Flags::User)
    )));
    assert_eq!(model.stop(Key::Mute), 0);
}

#[test]
fn pressed_key_writes_its_stop_time_before_the_reads() {
    for flags in [Flags::User, Flags::Mix] {
        for key in [Key::Mute, Key::Talk] {
            let mut state = base();
            state.key_enabled[idx(key)] = true;
            state.delay[idx(key)] = 300;
            match flags {
                Flags::User => state.user_pressed[idx(key)] = true,
                Flags::Mix => state.pressed[idx(key)] = true,
            }
            let model = Model::new(state);
            let muted = match flags {
                Flags::User => source_muted(&model, (), T),
                Flags::Mix => get_source_volume(&model, (), T) == 0.0,
            };
            // Pressed: push-to-mute mutes, push-to-talk lets it through.
            assert_eq!(muted, key == Key::Mute, "{flags:?} {key:?}");
            assert_eq!(model.stop(key), T + 300 * MS);
            let events = model.events();
            let other = match key {
                Key::Mute => Key::Talk,
                Key::Talk => Key::Mute,
            };
            let mut writes = vec![Event::KeyEnabled(Key::Mute)];
            if key == Key::Mute {
                writes.extend([
                    Event::Pressed(flags, Key::Mute),
                    Event::Delay(Key::Mute),
                    Event::SetStopTime(Key::Mute, T + 300 * MS),
                    Event::KeyEnabled(Key::Talk),
                ]);
            } else {
                writes.extend([
                    Event::KeyEnabled(Key::Talk),
                    Event::Pressed(flags, Key::Talk),
                    Event::Delay(Key::Talk),
                    Event::SetStopTime(Key::Talk, T + 300 * MS),
                ]);
            }
            assert_eq!(&events[..writes.len()], writes, "{flags:?} {key:?}");
            // The pressed key's stop time is not read: it is pressed.
            assert!(!events.contains(&Event::StopTime(key)), "{flags:?} {key:?}");
            assert!(events.contains(&Event::StopTime(other)));
            assert_eq!(model.stop(other), 0);
        }
    }
}

#[test]
fn key_not_enabled_is_neither_written_nor_obeyed() {
    for flags in [Flags::User, Flags::Mix] {
        let mut state = base();
        state.delay = [300, 300];
        state.stop = [u64::MAX, 0];
        match flags {
            Flags::User => state.user_pressed = [true, true],
            Flags::Mix => state.pressed = [true, true],
        }
        let model = Model::new(state);
        let muted = match flags {
            Flags::User => source_muted(&model, (), T),
            Flags::Mix => get_source_volume(&model, (), T) == 0.0,
        };
        assert!(!muted, "{flags:?}");
        let events = model.events();
        assert!(
            events
                .iter()
                .all(|e| !matches!(e, Event::SetStopTime(..) | Event::Delay(_))),
            "{flags:?} {events:?}"
        );
        assert_eq!(model.stop(Key::Mute), u64::MAX);
        assert_eq!(model.stop(Key::Talk), 0);
    }
}

#[test]
fn released_key_holds_strictly_before_its_stop_time() {
    for flags in [Flags::User, Flags::Mix] {
        for key in [Key::Mute, Key::Talk] {
            for (stop, held) in [
                (0, false),
                (T - 1, false),
                (T, false),
                (T + 1, true),
                (u64::MAX, true),
            ] {
                let mut state = base();
                state.key_enabled[idx(key)] = true;
                state.stop[idx(key)] = stop;
                let model = Model::new(state);
                let muted = match flags {
                    Flags::User => source_muted(&model, (), T),
                    Flags::Mix => get_source_volume(&model, (), T) == 0.0,
                };
                // Push-to-mute mutes while held; push-to-talk while not.
                assert_eq!(
                    muted,
                    held == (key == Key::Mute),
                    "{flags:?} {key:?} {stop}"
                );
                assert_eq!(model.stop(key), stop);
            }
        }
    }
}

#[test]
fn stop_time_saturates() {
    for (os_time, delay, stop) in [
        (T, u64::MAX / MS + 1, u64::MAX),
        (T, u64::MAX / MS, u64::MAX),
        (0, u64::MAX / MS, u64::MAX / MS * MS),
        (u64::MAX, 0, u64::MAX),
        (0, 0, 0),
    ] {
        for flags in [Flags::User, Flags::Mix] {
            let mut state = base();
            state.key_enabled = [true, true];
            state.delay = [delay, delay];
            match flags {
                Flags::User => state.user_pressed = [true, true],
                Flags::Mix => state.pressed = [true, true],
            }
            let model = Model::new(state);
            match flags {
                Flags::User => assert!(source_muted(&model, (), os_time)),
                Flags::Mix => assert_eq!(get_source_volume(&model, (), os_time).to_bits(), 0),
            }
            assert_eq!(model.stop(Key::Mute), stop);
            assert_eq!(model.stop(Key::Talk), stop);
        }
    }
}

#[test]
fn disabled_source_still_writes_its_stop_times() {
    for flags in [Flags::User, Flags::Mix] {
        let mut state = base();
        state.enabled = false;
        state.key_enabled = [true, true];
        state.delay = [300, 0];
        match flags {
            Flags::User => state.user_pressed = [true, true],
            Flags::Mix => state.pressed = [true, true],
        }
        let model = Model::new(state);
        match flags {
            Flags::User => assert!(source_muted(&model, (), T)),
            Flags::Mix => assert_eq!(get_source_volume(&model, (), T).to_bits(), 0),
        }
        assert_eq!(model.stop(Key::Mute), T + 300 * MS);
        assert_eq!(model.stop(Key::Talk), T);
    }
}

#[test]
fn push_to_mute_short_circuits_before_push_to_talk() {
    for flags in [Flags::User, Flags::Mix] {
        let mut state = base();
        state.key_enabled = [true, true];
        state.stop = [u64::MAX, 0];
        let model = Model::new(state);
        match flags {
            Flags::User => assert!(source_muted(&model, (), T)),
            Flags::Mix => assert_eq!(get_source_volume(&model, (), T).to_bits(), 0),
        }
        // Both keys enabled: each write check reads the pressed flag.
        let mut reads = vec![
            Event::KeyEnabled(Key::Mute),
            Event::Pressed(flags, Key::Mute),
            Event::KeyEnabled(Key::Talk),
            Event::Pressed(flags, Key::Talk),
            Event::Pressed(flags, Key::Mute),
            Event::StopTime(Key::Mute),
            Event::Pressed(flags, Key::Talk),
            Event::StopTime(Key::Talk),
            Event::Enabled,
            Event::Muted(flags),
            Event::KeyEnabled(Key::Mute),
        ];
        if flags == Flags::Mix {
            reads.push(Event::Volume);
        }
        assert_eq!(model.events(), reads, "{flags:?}");
    }
}

#[test]
fn volume_is_read_once_and_snapped() {
    for (volume, muted, expected) in [
        (0.5f32, false, 0.5f32),
        (0.5, true, 0.0),
        (1.00005, false, 1.0),
        (0.00005, false, 0.0),
        (-0.0, false, 0.0),
        (20.0, false, 20.0),
        (f32::NAN, true, 0.0),
    ] {
        let model = Model::new(State {
            volume,
            muted,
            ..base()
        });
        let got = get_source_volume(&model, (), T);
        assert_eq!(got.to_bits(), expected.to_bits(), "{volume} {muted}");
        let events = model.events();
        assert_eq!(events.iter().filter(|e| **e == Event::Volume).count(), 1);
        assert_eq!(events.last(), Some(&Event::Volume));
    }
    // NaN, not muted, keeps its bits.
    let model = Model::new(State {
        volume: f32::NAN,
        ..base()
    });
    assert_eq!(
        get_source_volume(&model, (), T).to_bits(),
        f32::NAN.to_bits()
    );
    // source_muted() never reads it.
    let model = Model::new(base());
    source_muted(&model, (), T);
    assert!(!model.events().contains(&Event::Volume));
}

/// The `test_source_muted_volume` capture cases, through `source_muted()`.
#[test]
fn mirrors_the_capture_cases() {
    struct Case {
        state: State,
        muted: bool,
    }
    let ptm = |f: fn(&mut State)| {
        let mut s = base();
        s.key_enabled[0] = true;
        f(&mut s);
        s
    };
    let ptt = |f: fn(&mut State)| {
        let mut s = base();
        s.key_enabled[1] = true;
        f(&mut s);
        s
    };
    let cases = [
        Case {
            state: base(),
            muted: false,
        },
        Case {
            state: State {
                enabled: false,
                ..base()
            },
            muted: true,
        },
        Case {
            state: State {
                user_muted: true,
                ..base()
            },
            muted: true,
        },
        Case {
            state: State {
                muted: true,
                ..base()
            },
            muted: false,
        },
        Case {
            state: ptm(|s| s.user_pressed[0] = true),
            muted: true,
        },
        Case {
            state: ptm(|s| s.stop[0] = u64::MAX),
            muted: true,
        },
        Case {
            state: ptm(|s| s.stop[0] = 1),
            muted: false,
        },
        Case {
            state: ptm(|s| s.pressed[0] = true),
            muted: false,
        },
        Case {
            state: ptt(|_| {}),
            muted: true,
        },
        Case {
            state: ptt(|s| s.user_pressed[1] = true),
            muted: false,
        },
        Case {
            state: ptt(|s| s.stop[1] = u64::MAX),
            muted: false,
        },
        Case {
            state: ptt(|s| s.pressed[1] = true),
            muted: true,
        },
    ];
    for (i, case) in cases.into_iter().enumerate() {
        let model = Model::new(case.state);
        assert_eq!(source_muted(&model, (), T), case.muted, "case {i}");
    }
}
