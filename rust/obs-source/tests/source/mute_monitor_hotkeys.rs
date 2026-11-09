use obs_source::mute_monitor_hotkeys::{Control, event};

#[derive(Debug, Default)]
struct Source {
    muted: bool,
    monitored: bool,
    monitor: bool,
    reads: usize,
    mute_signals: usize,
    monitor_signals: usize,
    // C audio-action type AUDIO_ACTION_MUTE, and its set value.
    actions: Vec<(u32, bool)>,
}
impl Control for Source {
    fn enabled(&mut self) -> bool {
        self.reads += 1;
        if self.monitor { self.monitored } else { self.muted }
    }
    fn set_enabled(&mut self, enabled: bool) {
        if self.monitor {
            self.monitored = enabled;
            self.monitor_signals += 1;
        } else {
            self.muted = enabled;
            self.mute_signals += 1;
            self.actions.push((1, enabled));
        }
    }
}

#[test]
fn mirrors_c_callback_matrix() {
    for key in 0..4 {
        let monitor = key >= 2;
        let target = key % 2 == 0;
        for initial in [false, true] {
            for other in [false, true] {
                for pressed in [false, true] {
                    let mut source = Source {
                        muted: if monitor { other } else { initial },
                        monitored: if monitor { initial } else { other },
                        monitor,
                        // Preserve an existing queued action just as the C fixture does.
                        actions: vec![(1, false)],
                        ..Source::default()
                    };
                    let changed = pressed && initial != target;
                    assert_eq!(event(&mut source, target, pressed), changed);
                    let expected = if changed { target } else { initial };
                    assert_eq!(source.muted, if monitor { other } else { expected });
                    assert_eq!(source.monitored, if monitor { expected } else { other });
                    assert_eq!(source.mute_signals, usize::from(!monitor && changed));
                    assert_eq!(source.monitor_signals, usize::from(monitor && changed));
                    assert_eq!(source.actions.len(), 1 + usize::from(!monitor && changed));
                    assert_eq!(source.actions[0], (1, false));
                    if !monitor && changed {
                        assert_eq!(source.actions[1], (1, target));
                    }
                    assert_eq!(source.reads, usize::from(pressed));
                }
            }
        }
    }
}

#[test]
fn mirrors_c_routed_pairs() {
    use obs_hotkey::pair::{Half, PairLatches, event as route};
    for monitor in [false, true] {
        let mut source = Source { monitor, ..Source::default() };
        let mut pair = PairLatches::default();
        {
        let mut trigger = |pair: &mut PairLatches, half: Half, pressed| {
            route(pair, half, pressed, |_| event(&mut source, half == Half::First, pressed));
        };
        trigger(&mut pair, Half::First, false);
        trigger(&mut pair, Half::Second, true);
        assert!(!pair.first);
        assert!(!pair.second);
        trigger(&mut pair, Half::First, true);
        assert!(pair.first);
        trigger(&mut pair, Half::First, true);
        trigger(&mut pair, Half::Second, true);
        assert!(pair.first);
        assert!(!pair.second);
        trigger(&mut pair, Half::First, false);
        assert!(!pair.first);
        // Inspect between events without holding the trigger's mutable borrow.
        }
        assert!(if monitor { source.monitored } else { source.muted });
        route(&mut pair, Half::Second, true, |_| event(&mut source, false, true));
        assert!(pair.second);
        route(&mut pair, Half::Second, false, |_| event(&mut source, false, false));
        assert!(!pair.second);
        assert!(!if monitor { source.monitored } else { source.muted });
    }
}
