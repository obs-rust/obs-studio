//! Tier 1 mirrors test_hotkey_enum.c's enumeration assertions with safe values.
use std::cell::RefCell;
use obs_hotkey::enumeration::{context, hotkeys, locked};

#[test]
fn test_hotkey_enum_live_order_and_early_stop() {
    for limit in [1, 16] {
        let mut seen = Vec::new();
        hotkeys(Some(0), |i| (i == 0).then_some(2), |i| {
            seen.push(i);
            seen.len() < limit
        });
        assert_eq!(seen, if limit == 1 { vec![0] } else { vec![0, 2] });
    }
    hotkeys::<usize>(None, |_| panic!("empty"), |_| panic!("empty"));
}

#[test]
fn test_hotkey_enum_context_skips_removed_and_keeps_bindings() {
    let ids = [0, 1, 2];
    let bindings = [vec![(2, 10), (1, 11)], vec![], vec![(1, 11)]];
    let mut saved = Vec::new();
    context(ids.len(), |i| ids[i], |id| (id != 1).then_some(id), |id| {
        saved.push((id, bindings[id].clone()));
        true
    });
    assert_eq!(saved, vec![(0, vec![(2, 10), (1, 11)]), (2, vec![(1, 11)])]);
    let mut seen = Vec::new();
    context(3, |i| ids[i], |id| (id != 1).then_some(id), |id| { seen.push(id); false });
    assert_eq!(seen, [0]);
    context::<usize>(0, |_| panic!("empty"), |_| panic!("empty"), |_| panic!("empty"));
}

#[test]
fn test_hotkey_enum_binding_indices_combinations_and_mutex() {
    for ok in [false, true] {
        for limit in [1, 16] {
            let trace = RefCell::new(Vec::new());
            let mut seen = Vec::new();
            let bindings = [(0, (2, 10)), (0, (1, 11)), (2, (1, 11))];
            locked(|| {trace.borrow_mut().push("lock"); ok}, || {
                for (index, binding) in bindings.iter().enumerate() {
                    assert_eq!(index, seen.len());
                    seen.push(*binding);
                    trace.borrow_mut().push("callback");
                    if seen.len() >= limit { break; }
                }
            }, || trace.borrow_mut().push("unlock"));
            assert_eq!(seen, if ok {bindings[..limit.min(3)].to_vec()} else {vec![]});
            let trace = trace.into_inner();
            assert_eq!(trace.first(), Some(&"lock"));
            assert_eq!(trace.last(), Some(&if ok {"unlock"} else {"lock"}));
        }
    }
}

#[test]
fn successor_is_read_before_callback_and_context_lookup_is_lazy() {
    let trace = RefCell::new(Vec::new());
    hotkeys(Some(0), |_| {trace.borrow_mut().push("next"); Some(1)}, |_| {trace.borrow_mut().push("callback"); false});
    assert_eq!(trace.into_inner(), ["next", "callback"]);
    let mut looked_up = Vec::new();
    context(4, |i| i, |id| {looked_up.push(id); (id != 0).then_some(id)}, |_| false);
    assert_eq!(looked_up, [0, 1]);
}
