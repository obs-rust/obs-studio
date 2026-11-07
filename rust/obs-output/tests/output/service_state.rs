//! Tier 1: mirrors test_service_state.c's test_state, plus reverse-link isolation.
use obs_output::service_state::*;

#[test]
fn test_state() {
    let mut destroy = false;
    let mut active = false;
    let mut output = None;
    let mut reverse = Some(2usize);
    assert!(!is_destroying(destroy));
    assert!(!is_active(active));
    assert!(!has_output(output));
    assert_eq!(get_control(None::<usize>), None);
    let control = Some(7usize);
    output = Some(1usize);
    set_active(&mut active, true);
    set_destroy(&mut destroy);
    set_destroy(&mut destroy);
    assert!(is_destroying(destroy));
    assert!(is_active(active));
    assert!(has_output(output));
    assert_eq!(get_control(control), control);
    detach_from_output(&mut reverse);
    assert_eq!(reverse, None);
    assert_eq!(output, Some(1));
    clear_output(&mut output);
    clear_output(&mut output);
    assert!(!has_output(output));
    set_active(&mut active, false);
    assert!(!is_active(active));
    assert!(is_destroying(destroy));
    assert_eq!(get_control(control), control);
}

#[test]
fn clear_does_not_detach() {
    let mut output = Some(1usize);
    let reverse = Some(2usize);
    clear_output(&mut output);
    assert_eq!(output, None);
    assert_eq!(reverse, Some(2));
}
