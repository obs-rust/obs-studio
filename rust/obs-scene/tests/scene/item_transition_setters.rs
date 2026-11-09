//! Safe-core coverage of the scene duplication characterization and edge cases.
use obs_scene::item_transition_setters::{Item, set_duration, set_transition};

#[derive(Debug, PartialEq)]
enum Event {
    Read(bool),
    Release(u8),
    Acquire(Option<u8>),
    Store(bool, Option<u8>),
}
#[derive(Default)]
struct SceneItem {
    transitions: [Option<u8>; 2],
    durations: [u32; 2],
    events: std::cell::RefCell<Vec<Event>>,
}
impl Item for SceneItem {
    type Source = u8;
    fn transition(&self, show: bool) -> Option<u8> {
        self.events.borrow_mut().push(Event::Read(show));
        self.transitions[usize::from(show)]
    }
    fn release(&mut self, source: u8) {
        self.events.borrow_mut().push(Event::Release(source));
    }
    fn get_ref(&mut self, source: Option<u8>) -> Option<u8> {
        self.events.borrow_mut().push(Event::Acquire(source));
        source.filter(|&s| s != 0)
    }
    fn store_transition(&mut self, show: bool, source: Option<u8>) {
        self.events.borrow_mut().push(Event::Store(show, source));
        self.transitions[usize::from(show)] = source;
    }
    fn store_duration(&mut self, show: bool, duration: u32) {
        self.durations[usize::from(show)] = duration;
    }
}

#[test]
fn private_copy_keeps_group_state_transition_fields() {
    let mut item = SceneItem::default();
    set_transition(Some(&mut item), true, Some(1));
    set_transition(Some(&mut item), false, Some(2));
    set_duration(Some(&mut item), true, 250);
    set_duration(Some(&mut item), false, 750);
    assert_eq!(item.transitions, [Some(2), Some(1)]);
    assert_eq!(item.durations, [750, 250]);
}

#[test]
fn replacement_order_and_selected_slot() {
    for show in [false, true] {
        for old in [None, Some(1)] {
            for new in [None, Some(0), Some(1), Some(2)] {
                let mut item = SceneItem {
                    transitions: [Some(3); 2],
                    durations: [250, 750],
                    ..Default::default()
                };
                item.transitions[usize::from(show)] = old;
                set_transition(Some(&mut item), show, new);
                let result = new.filter(|&s| s != 0);
                let mut expected = vec![Event::Read(show)];
                if let Some(old) = old {
                    expected.push(Event::Release(old));
                }
                expected.extend([Event::Acquire(new), Event::Store(show, result)]);
                assert_eq!(*item.events.borrow(), expected);
                assert_eq!(item.transitions[usize::from(show)], result);
                assert_eq!(item.transitions[usize::from(!show)], Some(3));
                assert_eq!(item.durations, [250, 750]);
            }
        }
    }
}

#[test]
fn duration_edges_and_null_items() {
    for show in [false, true] {
        set_transition::<SceneItem>(None, show, Some(1));
        set_duration::<SceneItem>(None, show, u32::MAX);
        let mut item = SceneItem {
            transitions: [Some(1), Some(2)],
            durations: [250, 750],
            ..Default::default()
        };
        for duration in [0, 1, 250, 750, u32::MAX] {
            let unselected = item.durations[usize::from(!show)];
            set_duration(Some(&mut item), show, duration);
            assert_eq!(item.durations[usize::from(show)], duration);
            assert_eq!(item.durations[usize::from(!show)], unselected);
            assert_eq!(item.transitions, [Some(1), Some(2)]);
            assert!(item.events.borrow().is_empty());
        }
    }
}
