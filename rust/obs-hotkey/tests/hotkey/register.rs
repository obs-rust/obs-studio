//! Safe-core mirrors of test_hotkey_register.c's three public characterization cases.
use obs_hotkey::register::{Kind, Registry, register};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub kind: Kind,
    pub weak: Option<usize>,
    pub context: Option<usize>,
    pub name: &'static str,
    pub description: &'static str,
    pub partner: usize,
    pub bindings: Vec<(&'static str, bool)>,
    pub presses: Vec<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct World {
    pub available: bool,
    pub private: bool,
    pub next: usize,
    pub events: Vec<&'static str>,
    pub entries: Vec<Entry>,
}
impl Default for World {
    fn default() -> Self {
        Self { available: true, private: false, next: 0, events: vec![], entries: vec![] }
    }
}
impl Registry for World {
    type Handle = usize;
    fn source_private(&mut self, _: usize) -> bool { self.events.push("private"); self.private }
    fn lock(&mut self) -> bool { self.events.push("lock"); self.available }
    fn weak(&mut self, _: Kind, owner: usize) -> usize { self.events.push("weak"); owner + 100 }
    fn context(&mut self, _: Kind, owner: usize) -> usize { self.events.push("context"); owner + 200 }
    fn insert(&mut self, kind: Kind, weak: Option<usize>, context: Option<usize>) -> usize {
        self.events.push("insert");
        let id = self.next;
        self.next = self.next.wrapping_add(1);
        self.entries.push(Entry {
            kind, weak, context, name: "registration.name", description: "registration description",
            partner: usize::MAX, bindings: if context.is_some() { vec![("OBS_KEY_A", true)] } else { vec![] }, presses: vec![],
        });
        id
    }
    fn unlock(&mut self) { self.events.push("unlock"); }
}

pub const OWNED: [Kind; 4] = [Kind::Source, Kind::Output, Kind::Encoder, Kind::Service];

#[test]
fn test_hotkey_register_unavailable_core() {
    let mut world = World { available: false, ..World::default() };
    assert_eq!(register(&mut world, Kind::Frontend, None), None);
    for kind in OWNED {
        assert_eq!(register(&mut world, kind, None), None);
        assert_eq!(register(&mut world, kind, Some(1)), None);
    }
    assert!(world.entries.is_empty());
    assert_eq!(world.next, 0);
    assert!(!world.events.contains(&"unlock"));
    assert!(!world.events.contains(&"weak"));
}

fn check_key(world: &mut World, id: usize, kind: Kind, owner: Option<usize>) {
    let key = &mut world.entries[id];
    assert_eq!(key.name, "registration.name");
    assert_eq!(key.description, "registration description");
    assert_eq!(key.kind, kind);
    assert_eq!(key.weak, owner.map(|o| o + 100));
    assert_eq!(key.context, owner.map(|o| o + 200));
    assert_eq!(key.partner, usize::MAX);
    // The existing engine owns callback dispatch. This dependency model pins
    // the observable press/release contract, also exercised by unchanged Tier 2.
    key.presses.push(true);
    assert_eq!(key.presses, [true]);
    key.presses.push(false);
    assert_eq!(key.presses, [true, false]);
}

#[test]
fn test_hotkey_register_frontend_and_admission() {
    let mut world = World::default();
    let id = register(&mut world, Kind::Frontend, None).unwrap();
    check_key(&mut world, id, Kind::Frontend, None);
    assert_eq!(world.events, ["lock", "insert", "unlock"]);
    world.events.clear();
    for kind in OWNED { assert_eq!(register(&mut world, kind, None), None); }
    assert!(world.events.is_empty()); // NULL never reaches a field accessor.
    world.private = true;
    assert_eq!(register(&mut world, Kind::Source, Some(1)), None);
    assert_eq!(world.events, ["private"]);
    let second = register(&mut world, Kind::Frontend, None).unwrap();
    assert_eq!(second, id + 1); // rejection consumes no IDs; duplicate names work.
    check_key(&mut world, second, Kind::Frontend, None);
    world.entries.clear(); // models the engine's explicit unregister.
    assert!(world.entries.is_empty());
}

#[test]
fn test_hotkey_register_owned_registration() {
    let mut world = World::default();
    let mut ids = vec![];
    for (owner, kind) in OWNED.into_iter().enumerate() {
        world.events.clear();
        let id = register(&mut world, kind, Some(owner + 1)).unwrap();
        assert!(!ids.contains(&id));
        ids.push(id);
        check_key(&mut world, id, kind, Some(owner + 1));
        assert_eq!(world.entries[id].bindings, [("OBS_KEY_A", true)]);
        let expected = if kind == Kind::Source {
            vec!["private", "lock", "weak", "context", "insert", "unlock"]
        } else { vec!["lock", "weak", "context", "insert", "unlock"] };
        assert_eq!(world.events, expected);
    }
    world.entries.clear(); // owner teardown removes its registered hotkeys.
    assert!(world.entries.is_empty());
}
