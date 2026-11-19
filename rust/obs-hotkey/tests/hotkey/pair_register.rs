//! Safe Tier 1 mirror of test_hotkey_pair_register.c. The backend models only
//! the unchanged pair engine (register_hotkey_pair_internal) and the owners;
//! Tier 2 runs the same assertions against the real engine and owners.
use obs_hotkey::pair_register::{INVALID_PAIR_ID, Kind, Registry, register};
use std::collections::BTreeMap;

pub const KINDS: [Kind; 4] = [Kind::Encoder, Kind::Output, Kind::Service, Kind::Source];

/// Callback tags: the first and second half's distinct active functions.
const FIRST: usize = 1;
const SECOND: usize = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub names: [String; 2],
    pub descriptions: [String; 2],
    pub callbacks: [Option<usize>; 2],
    pub data: [Option<usize>; 2],
}

fn request(data: [Option<usize>; 2]) -> Request {
    Request {
        names: ["object.on".into(), "object.off".into()],
        descriptions: ["Enable object".into(), "Disable object".into()],
        callbacks: [Some(FIRST), Some(SECOND)],
        data,
    }
}

/// The fields adapter's result: the entry point's weak helper and the context.
type Fields = (Kind, usize);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Fields(Kind, usize),
    Register(Kind, usize, Fields, Request),
}

struct Owner {
    kind: Kind,
    weak_refs: usize,
    saved: BTreeMap<String, String>,
    alive: bool,
}

#[derive(Debug)]
struct Key {
    name: String,
    description: String,
    kind: Kind,
    registerer: usize,
    context: usize,
    partner: usize,
    pair: usize,
    half: usize,
    bindings: Vec<String>,
}

struct Pair {
    callbacks: [Option<usize>; 2],
    data: [Option<usize>; 2],
}

#[derive(Default, Clone, Copy)]
struct Calls {
    half: usize,
    calls: usize,
    pair: usize,
    key: usize,
    pressed: bool,
}

#[derive(Default)]
struct Engine {
    available: bool,
    next_pair: usize,
    next_key: usize,
    events: Vec<Event>,
    owners: BTreeMap<usize, Owner>,
    keys: BTreeMap<usize, Key>,
    pairs: BTreeMap<usize, Pair>,
    calls: BTreeMap<usize, Calls>,
}

const fn weak_of(owner: usize) -> usize {
    owner + 100
}
const fn context_of(owner: usize) -> usize {
    owner + 200
}

impl Engine {
    fn live() -> Self {
        Self {
            available: true,
            ..Self::default()
        }
    }

    fn create(&mut self, owner: usize, kind: Kind, saved: &[(&str, &str)]) {
        self.owners.insert(
            owner,
            Owner {
                kind,
                weak_refs: 0,
                saved: saved
                    .iter()
                    .map(|(name, key)| ((*name).into(), (*key).into()))
                    .collect(),
                alive: true,
            },
        );
    }

    fn weak(&mut self, owner: usize) -> usize {
        self.owners.get_mut(&owner).unwrap().weak_refs += 1;
        weak_of(owner)
    }

    fn weak_release(&mut self, owner: usize) {
        self.owners.get_mut(&owner).unwrap().weak_refs -= 1;
    }

    fn pair_keys(&self) -> [Option<usize>; 2] {
        let find = |name| {
            self.keys
                .iter()
                .find(|(_, key)| key.name == name)
                .map(|(id, _)| *id)
        };
        [find("object.on"), find("object.off")]
    }

    fn set_calls(&mut self, data: usize, calls: Calls) {
        self.calls.insert(data, calls);
    }

    /// obs_hotkey_trigger_routed_callback through the pair's half functions.
    fn trigger(&mut self, id: usize, pressed: bool) {
        let key = &self.keys[&id];
        let pair = &self.pairs[&key.pair];
        let half = key.half;
        let callback = pair.callbacks[half].unwrap();
        let data = pair.data[half].unwrap();
        let (pair_id, key_id) = (key.pair, id);
        let call = self.calls.get_mut(&data).unwrap();
        // record_call: the callback's own half, pair and key are forwarded.
        assert_eq!(call.half, callback - 1);
        assert_eq!(call.pair, pair_id);
        assert_eq!(call.key, key_id);
        call.calls += 1;
        call.pressed = pressed;
    }

    fn save(&self, owner: usize) -> BTreeMap<String, Vec<String>> {
        self.keys
            .values()
            .filter(|key| key.context == context_of(owner))
            .filter(|key| !key.bindings.is_empty())
            .map(|key| (key.name.clone(), key.bindings.clone()))
            .collect()
    }

    fn unregister_pair(&mut self, pair: usize) {
        if self.pairs.remove(&pair).is_some() {
            self.keys.retain(|_, key| key.pair != pair);
        }
    }

    /// The owner's destroy path removes its context's keys and pairs.
    fn release(&mut self, owner: usize) {
        self.owners.get_mut(&owner).unwrap().alive = false;
        let context = context_of(owner);
        let pairs: Vec<usize> = self
            .keys
            .values()
            .filter(|key| key.context == context)
            .map(|key| key.pair)
            .collect();
        for pair in pairs {
            self.unregister_pair(pair);
        }
    }
}

impl Registry<Request> for Engine {
    type Owner = usize;
    type Fields = Fields;

    fn fields(&mut self, kind: Kind, owner: usize) -> Fields {
        let state = &self.owners[&owner];
        assert!(state.alive);
        assert_eq!(state.kind, kind, "the entry point names the owner's type");
        self.events.push(Event::Fields(kind, owner));
        (kind, context_of(owner))
    }

    fn register(&mut self, kind: Kind, owner: usize, fields: Fields, request: Request) -> usize {
        self.events
            .push(Event::Register(kind, owner, fields, request.clone()));
        if !self.available {
            return INVALID_PAIR_ID;
        }
        let (helper, context) = fields;
        let pair = self.next_pair;
        self.next_pair += 1;
        self.pairs.insert(
            pair,
            Pair {
                callbacks: request.callbacks,
                data: request.data,
            },
        );
        let mut ids = [0; 2];
        for (half, slot) in ids.iter_mut().enumerate() {
            // Each half acquires its own weak reference through the helper.
            assert_eq!(helper, self.owners[&owner].kind);
            let registerer = self.weak(owner);
            let id = self.next_key;
            self.next_key += 1;
            let name = request.names[half].clone();
            let bindings = self.owners[&owner]
                .saved
                .get(&name)
                .cloned()
                .into_iter()
                .collect();
            self.keys.insert(
                id,
                Key {
                    name,
                    description: request.descriptions[half].clone(),
                    kind,
                    registerer,
                    context,
                    partner: usize::MAX,
                    pair,
                    half,
                    bindings,
                },
            );
            *slot = id;
        }
        self.keys.get_mut(&ids[0]).unwrap().partner = ids[1];
        self.keys.get_mut(&ids[1]).unwrap().partner = ids[0];
        pair
    }
}

fn null_owners(engine: &mut Engine) {
    for kind in KINDS {
        assert_eq!(
            register(engine, kind, None, request([None, None])),
            INVALID_PAIR_ID
        );
    }
    assert!(engine.events.is_empty());
}

fn check_saved(engine: &Engine, owner: usize, other: usize) {
    let saved = engine.save(owner);
    assert_eq!(saved["object.on"], ["OBS_KEY_A"]);
    assert_eq!(saved["object.off"], ["OBS_KEY_B"]);
    let saved = engine.save(other);
    assert!(!saved.contains_key("object.on"));
    assert!(!saved.contains_key("object.off"));
}

// One half's identity, partner and routed callback, as in the C loop.
fn check_half(
    engine: &mut Engine,
    kind: Kind,
    weak: usize,
    pair: usize,
    ids: [usize; 2],
    data: [usize; 2],
    half: usize,
) {
    let (id, partner) = (ids[half], ids[1 - half]);
    let (mine, other) = (data[half], data[1 - half]);
    let key = &engine.keys[&id];
    assert_eq!(key.kind, kind);
    assert_eq!(key.registerer, weak);
    assert_eq!(key.partner, partner);
    let mut calls = engine.calls[&mine];
    calls.pair = pair;
    calls.key = id;
    engine.set_calls(mine, calls);
    engine.trigger(id, true);
    assert_eq!(engine.calls[&mine].calls, 1);
    assert!(engine.calls[&mine].pressed);
    assert_eq!(engine.calls[&other].calls, if half == 0 { 0 } else { 2 });
    engine.trigger(id, false);
    assert_eq!(engine.calls[&mine].calls, 2);
    assert!(!engine.calls[&mine].pressed);
}

#[test]
fn test_hotkey_pair_register_null_owners() {
    let mut engine = Engine::default();
    null_owners(&mut engine);
    assert!(engine.keys.is_empty() && engine.pairs.is_empty());
}

#[test]
fn test_hotkey_pair_register_object_registration() {
    let mut engine = Engine::live();
    // NULL owners must neither enroll keys nor consume IDs with a live core.
    null_owners(&mut engine);
    assert_eq!((engine.next_pair, engine.next_key), (0, 0));
    for (i, kind) in KINDS.into_iter().enumerate() {
        let (owner, other) = (2 * i + 1, 2 * i + 2);
        engine.create(
            owner,
            kind,
            &[("object.on", "OBS_KEY_A"), ("object.off", "OBS_KEY_B")],
        );
        engine.create(other, kind, &[]);
        let weak = engine.weak(owner);
        let (data0, data1) = (1000 + 2 * i, 1001 + 2 * i);
        engine.set_calls(data0, Calls::default());
        engine.set_calls(
            data1,
            Calls {
                half: 1,
                ..Calls::default()
            },
        );
        let mut args = request([Some(data0), Some(data1)]);
        engine.events.clear();
        let pair = register(&mut engine, kind, Some(owner), args.clone());
        assert_ne!(pair, INVALID_PAIR_ID);
        if i == 0 {
            assert_eq!(pair, 0);
        }
        assert_eq!(
            engine.events,
            [
                Event::Fields(kind, owner),
                Event::Register(kind, owner, (kind, context_of(owner)), args.clone()),
            ]
        );
        let found = engine.pair_keys();
        let ids = [found[0].unwrap(), found[1].unwrap()];
        assert_ne!(ids[0], ids[1]);
        if i == 0 {
            assert_eq!(ids, [0, 1]);
        }
        // Metadata is copied, and each half acquired its own weak reference.
        for text in args.names.iter_mut().chain(args.descriptions.iter_mut()) {
            text.replace_range(0..1, "X");
        }
        assert_eq!(engine.owners[&owner].weak_refs, 3);
        for half in 0..2 {
            check_half(&mut engine, kind, weak, pair, ids, [data0, data1], half);
        }
        assert_eq!(engine.keys[&ids[0]].name, "object.on");
        assert_eq!(engine.keys[&ids[1]].name, "object.off");
        assert_eq!(engine.keys[&ids[0]].description, "Enable object");
        assert_eq!(engine.keys[&ids[1]].description, "Disable object");
        check_saved(&engine, owner, other);
        engine.unregister_pair(pair);
        assert_eq!(engine.pair_keys(), [None, None]);
        // Leave a second pair registered for the owner's destroy path.
        let pair = register(
            &mut engine,
            kind,
            Some(owner),
            request([Some(data0), Some(data1)]),
        );
        assert_ne!(pair, INVALID_PAIR_ID);
        engine.release(owner);
        engine.release(other);
        assert_eq!(engine.pair_keys(), [None, None]);
        engine.weak_release(owner);
        // Destroy removed the context's pair as well as its two keys.
        assert!(!engine.pairs.contains_key(&pair));
        engine.unregister_pair(pair);
    }
}

#[test]
fn test_hotkey_pair_register_private_source_and_null_callbacks() {
    // Unlike single-hotkey source registration, pair registration admits a
    // private source (characterized, not endorsed) and NULL callbacks/data.
    let mut engine = Engine::live();
    engine.create(9, Kind::Source, &[]);
    let args = Request {
        callbacks: [None, None],
        ..request([None, None])
    };
    let pair = register(&mut engine, Kind::Source, Some(9), args);
    assert_ne!(pair, INVALID_PAIR_ID);
    let weak = engine.weak(9);
    for id in engine.pair_keys() {
        let key = &engine.keys[&id.unwrap()];
        assert_eq!(key.kind, Kind::Source);
        assert_eq!(key.registerer, weak);
    }
    assert_eq!(engine.pairs[&pair].callbacks, [None, None]);
    assert_eq!(engine.pairs[&pair].data, [None, None]);
    engine.release(9);
    assert_eq!(engine.pair_keys(), [None, None]);
    engine.weak_release(9);
}

#[test]
fn engine_result_is_returned_unchanged_including_invalid() {
    // Without a core the owner's fields are still read, then the engine's
    // failed lock yields the invalid ID and nothing is enrolled.
    let mut engine = Engine::default();
    engine.create(3, Kind::Output, &[]);
    let args = request([None, None]);
    assert_eq!(
        register(&mut engine, Kind::Output, Some(3), args.clone()),
        INVALID_PAIR_ID
    );
    assert_eq!(
        engine.events,
        [
            Event::Fields(Kind::Output, 3),
            Event::Register(Kind::Output, 3, (Kind::Output, context_of(3)), args),
        ]
    );
    assert!(engine.keys.is_empty());
    let mut engine = Engine::live();
    engine.next_pair = usize::MAX - 1;
    engine.create(4, Kind::Encoder, &[]);
    assert_eq!(
        register(&mut engine, Kind::Encoder, Some(4), request([None, None])),
        usize::MAX - 1
    );
}
