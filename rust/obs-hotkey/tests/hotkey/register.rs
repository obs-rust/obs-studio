//! Safe Tier 1 mirror of test_hotkey_register.c. This backend models only the
//! unchanged engine dependencies; Tier 2 retains the real engine and owners.
use obs_hotkey::register::{Kind, Registry, register};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub name: String,
    pub description: String,
    pub callback: usize,
    pub data: usize,
}

pub fn request() -> Request {
    Request {
        name: "registration.name".into(),
        description: "registration description".into(),
        callback: 71,
        data: 92,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Private(usize),
    Lock,
    Acquire(Kind, usize),
    Register(Kind, Option<(usize, usize)>, Request),
    Unlock,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Binding {
    key: String,
    control: bool,
}

#[derive(Debug)]
struct Key {
    id: usize,
    kind: Kind,
    owner: Option<(usize, usize)>,
    request: Request,
    pair_partner: usize,
    bindings: Vec<Binding>,
}

#[derive(Default)]
struct Observation {
    id: usize,
    key: Option<usize>,
    calls: usize,
    pressed: bool,
}

type Callback = fn(&mut Observation, usize, usize, bool);
type Saved = BTreeMap<String, Vec<Binding>>;

fn callback(seen: &mut Observation, id: usize, key: usize, pressed: bool) {
    seen.id = id;
    seen.key = Some(key);
    seen.calls += 1;
    seen.pressed = pressed;
}

#[derive(Default)]
pub struct Engine {
    pub available: bool,
    pub private: bool,
    pub next: usize,
    pub events: Vec<Event>,
    keys: BTreeMap<usize, Key>,
    owners: BTreeSet<usize>,
    saved: BTreeMap<usize, Saved>,
    callbacks: BTreeMap<usize, Callback>,
    observations: BTreeMap<usize, Observation>,
    held: bool,
}

impl Engine {
    fn live(next: usize) -> Self {
        Self {
            available: true,
            next,
            callbacks: BTreeMap::from([(71, callback as Callback)]),
            ..Self::default()
        }
    }

    fn seed_owner(&mut self, owner: usize) {
        self.owners.insert(owner);
        self.saved.insert(
            owner + 200,
            BTreeMap::from([(
                "registration.name".into(),
                vec![Binding {
                    key: "OBS_KEY_A".into(),
                    control: true,
                }],
            )]),
        );
    }

    fn lookup(&self, id: usize) -> Option<&Key> {
        self.keys.get(&id)
    }

    fn trigger(&mut self, id: usize, pressed: bool) {
        let key = self.keys.get(&id).unwrap();
        let func = self.callbacks[&key.request.callback];
        func(
            self.observations.entry(key.request.data).or_default(),
            id,
            key.id,
            pressed,
        );
    }

    fn save(&self, id: usize) -> Option<&[Binding]> {
        self.keys.get(&id).map(|key| key.bindings.as_slice())
    }

    fn save_owner(&self, owner: usize) -> Option<Saved> {
        self.owners.contains(&owner).then(|| {
            self.keys
                .values()
                .filter(|key| key.owner.is_some_and(|(_, context)| context == owner + 200))
                .map(|key| (key.request.name.clone(), key.bindings.clone()))
                .collect()
        })
    }

    fn release_owner(&mut self, owner: usize) {
        self.owners.remove(&owner);
        self.keys
            .retain(|_, key| !key.owner.is_some_and(|(_, context)| context == owner + 200));
    }

    fn weak_owner(&self, weak: usize) -> Option<usize> {
        weak.checked_sub(100)
            .filter(|owner| self.owners.contains(owner))
    }
}

impl Registry<Request> for Engine {
    type Owner = usize;
    type Weak = usize;
    type Context = usize;
    fn source_private(&mut self, owner: usize) -> bool {
        self.events.push(Event::Private(owner));
        self.private
    }
    fn lock(&mut self) -> bool {
        self.events.push(Event::Lock);
        assert!(!self.held);
        self.held = self.available;
        self.available
    }
    fn acquire(&mut self, kind: Kind, owner: usize) -> (usize, usize) {
        assert!(self.held);
        assert!(self.owners.contains(&owner));
        self.events.push(Event::Acquire(kind, owner));
        (owner + 100, owner + 200)
    }
    fn register(&mut self, kind: Kind, owner: Option<(usize, usize)>, request: Request) -> usize {
        assert!(self.held);
        self.events
            .push(Event::Register(kind, owner, request.clone()));
        let id = self.next;
        self.next = self.next.wrapping_add(1);
        let bindings = owner
            .and_then(|(_, context)| self.saved.get(&context))
            .and_then(|saved| saved.get(&request.name))
            .cloned()
            .unwrap_or_default();
        self.keys.insert(
            id,
            Key {
                id,
                kind,
                owner,
                request,
                pair_partner: usize::MAX,
                bindings,
            },
        );
        id
    }
    fn unlock(&mut self) {
        assert!(self.held);
        self.events.push(Event::Unlock);
        self.held = false;
    }
}

pub const KINDS: [Kind; 5] = [
    Kind::Frontend,
    Kind::Source,
    Kind::Output,
    Kind::Encoder,
    Kind::Service,
];

// check_key in the C characterization: safe IDs also serve as opaque key
// handles here, so identity assertions need no raw pointer or unsafe code.
fn check_key(engine: &mut Engine, id: usize, kind: Kind, weak: Option<usize>, data: usize) {
    assert_ne!(id, usize::MAX);
    let key = engine.lookup(id).unwrap();
    assert_eq!(key.id, id);
    assert_eq!(key.request.name, "registration.name");
    assert_eq!(key.request.description, "registration description");
    assert_eq!(key.kind, kind);
    assert_eq!(key.owner.map(|(weak, _)| weak), weak);
    assert_eq!(key.pair_partner, usize::MAX);
    engine.trigger(id, true);
    let seen = &engine.observations[&data];
    assert_eq!(seen.calls, 1);
    assert_eq!(seen.id, id);
    assert_eq!(seen.key, Some(id));
    assert!(seen.pressed);
    engine.trigger(id, false);
    let seen = &engine.observations[&data];
    assert_eq!(seen.calls, 2);
    assert!(!seen.pressed);
}

// check_bindings in the C characterization: per-owner and direct saved data.
fn check_bindings(engine: &Engine, owner: usize, id: usize) {
    let saved = engine.save_owner(owner).unwrap();
    let array = saved.get("registration.name").unwrap();
    assert_eq!(array.len(), 1);
    let direct = engine.save(id).unwrap();
    assert_eq!(direct.len(), 1);
    assert_eq!(direct[0].key, "OBS_KEY_A");
    assert!(direct[0].control);
}

#[test]
fn test_hotkey_register_unavailable_core() {
    for kind in KINDS {
        let mut engine = Engine::default();
        assert_eq!(register(&mut engine, kind, None, request()), usize::MAX);
        assert!(engine.keys.is_empty());
        assert_eq!(
            engine.events,
            if kind == Kind::Frontend {
                vec![Event::Lock]
            } else {
                vec![]
            }
        );
        engine.events.clear();
        assert_eq!(register(&mut engine, kind, Some(7), request()), usize::MAX);
        assert!(
            !engine
                .events
                .iter()
                .any(|event| matches!(event, Event::Acquire(..) | Event::Unlock))
        );
    }
}

#[test]
fn test_hotkey_register_frontend_and_admission() {
    let mut engine = Engine::live(31);
    let first = register(&mut engine, Kind::Frontend, None, request());
    assert_eq!(
        engine.events,
        [
            Event::Lock,
            Event::Register(Kind::Frontend, None, request()),
            Event::Unlock
        ]
    );
    check_key(&mut engine, first, Kind::Frontend, None, 92);
    for kind in KINDS.into_iter().skip(1) {
        engine.events.clear();
        assert_eq!(register(&mut engine, kind, None, request()), usize::MAX);
        assert!(engine.events.is_empty());
    }
    engine.seed_owner(4);
    engine.private = true;
    assert_eq!(
        register(&mut engine, Kind::Source, Some(4), request()),
        usize::MAX
    );
    assert_eq!(engine.events, [Event::Private(4)]);
    engine.release_owner(4);
    let second = register(&mut engine, Kind::Frontend, None, request());
    assert_eq!(second, first + 1);
    assert!(engine.lookup(second).is_some());
    engine.keys.remove(&first);
    engine.keys.remove(&second);
    assert!(engine.lookup(first).is_none());
    assert!(engine.lookup(second).is_none());
}

#[test]
fn test_hotkey_register_owned_registration() {
    let mut engine = Engine::live(61);
    let mut ids = Vec::new();
    for (offset, kind) in KINDS.into_iter().skip(1).enumerate() {
        let owner = offset + 1;
        engine.seed_owner(owner);
        engine.events.clear();
        let mut args = request();
        args.data += offset;
        let id = register(&mut engine, kind, Some(owner), args.clone());
        assert_eq!(id, 61 + offset);
        assert_eq!(
            engine.lookup(id).unwrap().owner,
            Some((owner + 100, owner + 200))
        );
        let mut events = vec![
            Event::Lock,
            Event::Acquire(kind, owner),
            Event::Register(kind, Some((owner + 100, owner + 200)), args),
            Event::Unlock,
        ];
        if kind == Kind::Source {
            events.insert(0, Event::Private(owner));
        }
        assert_eq!(engine.events, events);
        check_key(&mut engine, id, kind, Some(owner + 100), 92 + offset);
        ids.push(id);
    }
    for (i, id) in ids.iter().enumerate() {
        for other in ids.iter().skip(i + 1) {
            assert_ne!(id, other);
        }
        check_bindings(&engine, i + 1, *id);
    }
    for owner in 1..=4 {
        engine.release_owner(owner);
    }
    for id in ids {
        assert!(engine.lookup(id).is_none());
    }
    for weak in 101..=104 {
        assert!(engine.weak_owner(weak).is_none());
    }
}

#[test]
fn engine_result_is_returned_unchanged_including_invalid() {
    let mut engine = Engine::live(usize::MAX);
    assert_eq!(
        register(&mut engine, Kind::Frontend, None, request()),
        usize::MAX
    );
    assert_eq!(engine.events.last(), Some(&Event::Unlock));
}
