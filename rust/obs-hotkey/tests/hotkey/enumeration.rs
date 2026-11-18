//! Tier 1: assertions from test_hotkey_enum.c over safe registry handles.
use obs_hotkey::enumeration::{self, Context, Registry};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Binding {
    id: usize,
    key: u32,
    modifiers: u32,
}

#[derive(Default)]
struct State {
    available: bool,
    depth: usize,
    locks: usize,
    unlocks: usize,
    keys: Vec<usize>,
    bindings: Vec<Binding>,
    ids: Vec<usize>,
}
#[derive(Clone, Default)]
struct World(Rc<RefCell<State>>);
impl Registry for World {
    type Hotkey = usize;
    type Binding = Binding;
    fn lock(&mut self) -> bool {
        let mut state = self.0.borrow_mut();
        state.locks += 1;
        if !state.available {
            return false;
        }
        state.depth += 1;
        true
    }
    fn unlock(&mut self) {
        let mut state = self.0.borrow_mut();
        assert!(state.depth > 0);
        state.depth -= 1;
        state.unlocks += 1;
    }
    fn first(&self) -> Option<usize> {
        self.0.borrow().keys.first().copied()
    }
    fn next(&self, key: usize) -> Option<usize> {
        let state = self.0.borrow();
        let pos = state.keys.iter().position(|&id| id == key).unwrap();
        state.keys.get(pos + 1).copied()
    }
    fn id(&self, key: usize) -> usize {
        assert!(self.0.borrow().keys.contains(&key));
        key
    }
    fn bindings(&mut self, callback: &mut dyn FnMut(usize, Binding) -> bool) {
        let bindings = self.0.borrow().bindings.clone();
        for (index, binding) in bindings.into_iter().enumerate() {
            if !callback(index, binding) {
                break;
            }
        }
    }
}
impl Context for World {
    type Hotkey = usize;
    type Snapshot = Vec<usize>;
    fn snapshot(&self) -> (usize, Vec<usize>) {
        let ids = self.0.borrow().ids.clone();
        (ids.len(), ids)
    }
    fn id(&self, ids: &Vec<usize>, index: usize) -> usize {
        ids[index]
    }
    fn find(&self, id: usize) -> Option<usize> {
        self.0.borrow().keys.contains(&id).then_some(id)
    }
}

#[test]
fn enumeration_contract_mirrors_c() {
    let mut world = World::default();
    enumeration::hotkeys(&mut world, |_, _| panic!("no core"));
    enumeration::bindings(&mut world, |_, _| panic!("no core"));
    {
        let mut state = world.0.borrow_mut();
        state.available = true;
        state.keys = vec![0, 2];
        state.ids = vec![0, 1, 2];
        state.bindings = vec![
            Binding { id: 0, key: 1, modifiers: 2 },
            Binding { id: 0, key: 2, modifiers: 4 },
            Binding { id: 2, key: 2, modifiers: 4 },
        ];
    }
    let mut ids = Vec::new();
    let observer = world.clone();
    enumeration::hotkeys(&mut world, |id, key| {
        assert_eq!(observer.0.borrow().depth, 1);
        assert_eq!(id, key);
        ids.push(id);
        true
    });
    assert_eq!(ids, [0, 2]);
    ids.clear();
    enumeration::hotkeys(&mut world, |id, _| { ids.push(id); false });
    assert_eq!(ids, [0]);
    let mut bindings = Vec::new();
    enumeration::bindings(&mut world, |index, binding| {
        assert_eq!(observer.0.borrow().depth, 1);
        assert_eq!(index, bindings.len());
        bindings.push(binding);
        true
    });
    assert_eq!(bindings, world.0.borrow().bindings);
    assert_eq!(bindings.iter().map(|b| b.id).collect::<Vec<_>>(), [0, 0, 2]);
    bindings.clear();
    enumeration::bindings(&mut world, |_, binding| { bindings.push(binding); false });
    assert_eq!(bindings.len(), 1);
    ids.clear();
    enumeration::context(&world, |key| { ids.push(key); true });
    assert_eq!(ids, [0, 2]);
    world.0.borrow_mut().keys.clear();
    world.0.borrow_mut().bindings.clear();
    enumeration::hotkeys(&mut world, |_, _| panic!("destroyed"));
    enumeration::bindings(&mut world, |_, _| panic!("destroyed"));
    assert_eq!(world.0.borrow().depth, 0);
    assert_eq!(world.0.borrow().locks, world.0.borrow().unlocks + 2);
}

#[test]
fn callbacks_can_remove_current_and_reenter() {
    let mut world = World::default();
    world.0.borrow_mut().available = true;
    world.0.borrow_mut().keys = vec![4, 5, 6];
    let observer = world.clone();
    let mut visited = Vec::new();
    enumeration::hotkeys(&mut world, |id, _| {
        visited.push(id);
        enumeration::hotkeys(&mut observer.clone(), |nested, _| {
            assert_eq!(observer.0.borrow().depth, 2);
            assert_eq!(nested, id);
            false
        });
        observer.0.borrow_mut().keys.retain(|&key| key != id);
        true
    });
    assert_eq!(visited, [4, 5, 6]);
    assert_eq!(world.0.borrow().depth, 0);
}

#[test]
fn context_looks_up_later_ids_after_callback_and_stops_early() {
    let world = World::default();
    world.0.borrow_mut().keys = vec![1, 2, 3];
    world.0.borrow_mut().ids = vec![99, 1, 2, 3];
    let mut visited = Vec::new();
    enumeration::context(&world, |key| {
        visited.push(key);
        world.0.borrow_mut().keys.retain(|&key| key != 2);
        true
    });
    assert_eq!(visited, [1, 3]);
    visited.clear();
    enumeration::context(&world, |key| { visited.push(key); false });
    assert_eq!(visited, [1]);
    assert_eq!(world.0.borrow().locks, 0);
}
