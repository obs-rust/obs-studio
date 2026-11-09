use obs_output::encoder_clear_roi::{RoiState, clear_roi};

#[derive(Default)]
struct State {
    count: usize,
    increment: u32,
    locks: usize,
    capacity: usize,
    emptied_before_lock: bool,
}
impl RoiState for State {
    fn count(&self) -> usize { self.count }
    fn with_locked_fields(&mut self, update: impl FnOnce(&mut usize, &mut u32)) {
        self.locks += 1;
        if self.emptied_before_lock { self.count = 0; }
        update(&mut self.count, &mut self.increment);
    }
}
fn clear(state: &mut State) { clear_roi(Some(state), || panic!("valid encoder")); }

#[test]
fn test_increment_after_add_and_clear() {
    let mut state = State::default();
    assert_eq!(state.increment, 0);
    assert_eq!(state.count, 0);
    clear(&mut state);
    assert_eq!(state.increment, 0);
    assert_eq!(state.locks, 0);
    // Two successful C adds each append and increment under the shared mutex.
    state.count = 2;
    state.capacity = 8;
    state.increment = 2;
    assert!(state.count > 0);
    clear(&mut state);
    assert_eq!(state.increment, 3);
    assert_eq!(state.count, 0);
    assert_eq!(state.capacity, 8);
    clear(&mut state);
    assert_eq!(state.increment, 3);
    assert_eq!(state.locks, 1);
    state.count += 1;
    state.increment += 1;
    assert_eq!(state.increment, 4);
}
#[test]
fn full_width_wrapping_and_no_locked_recheck() {
    let mut state = State { count: usize::MAX, increment: u32::MAX,
        emptied_before_lock: true, ..State::default() };
    clear(&mut state);
    assert_eq!(state.count, 0);
    assert_eq!(state.increment, 0);
    assert_eq!(state.locks, 1);
}
#[test]
fn test_null_encoder() {
    let mut logs = 0;
    clear_roi(None::<&mut State>, || logs += 1);
    assert_eq!(logs, 1);
}
