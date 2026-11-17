use obs_scene::item_hotkeys::{Item, Rename, acquire, description, rename, trigger};

#[derive(Default)]
struct Model {
    owners: i64,
    visible: bool,
    changes: usize,
    releases: usize,
    trace: Vec<&'static str>,
    race: Option<i64>,
}
impl Item for Model {
    fn owners(&mut self) -> i64 {
        self.trace.push("load");
        self.owners
    }
    fn compare_owners(&mut self, expected: i64, next: i64) -> Result<(), i64> {
        self.trace.push("cas");
        if let Some(observed) = self.race.take() {
            self.owners = observed;
        }
        if self.owners == expected {
            self.owners = next;
            Ok(())
        } else {
            Err(self.owners)
        }
    }
    fn visible(&mut self) -> bool {
        self.trace.push("visible");
        self.visible
    }
    fn set_visible(&mut self, visible: bool) {
        self.trace.push("set");
        self.visible = visible;
        self.changes += 1;
    }
    fn release(&mut self, acquired: bool) {
        self.trace.push("release");
        if acquired {
            self.owners -= 1;
            self.releases += 1;
        }
    }
}

#[test]
fn test_scene_item_hotkeys_trigger_visibility() {
    let mut item = Model {
        owners: 2,
        visible: true,
        ..Model::default()
    };
    assert!(!trigger(&mut item, true, false));
    assert!(!trigger(&mut item, false, false));
    assert!(!trigger(&mut item, true, true));
    assert!(item.visible);
    assert_eq!(item.changes, 0);
    let mut latched = None;
    // Mirror the public paired callback routing: only handled presses latch.
    let route = |item: &mut Model, latched: &mut Option<bool>, show, pressed| {
        if pressed && latched.is_some() {
            return;
        }
        if trigger(item, show, pressed) {
            *latched = Some(show);
        }
        if !pressed && *latched == Some(show) {
            *latched = None;
        }
    };
    for i in 0..32 {
        route(&mut item, &mut latched, false, true);
        assert!(!item.visible);
        assert_eq!(item.changes, 2 * i + 1);
        route(&mut item, &mut latched, true, true);
        assert!(!item.visible);
        route(&mut item, &mut latched, false, false);
        route(&mut item, &mut latched, false, true);
        route(&mut item, &mut latched, true, true);
        assert!(item.visible);
        assert_eq!(item.changes, 2 * i + 2);
        route(&mut item, &mut latched, false, true);
        assert!(item.visible);
        route(&mut item, &mut latched, true, false);
        assert_eq!(item.owners, 2);
    }
    assert_eq!(item.releases, 3 + 32 * 5);
}

#[test]
fn test_scene_item_hotkeys_renamed_descriptions() {
    for _item in 0..2 {
        assert_eq!(
            description(b"Show %1", b"original"),
            Some(b"Show original".to_vec())
        );
        assert_eq!(
            description(b"Hide %1", b"original"),
            Some(b"Hide original".to_vec())
        );
        assert_eq!(
            description(b"[%1] show %1", b"renamed"),
            Some(b"[renamed] show renamed".to_vec())
        );
        assert_eq!(
            description(b"hide <%1>", b"renamed"),
            Some(b"hide <renamed>".to_vec())
        );
        assert_eq!(description(b"literal", b"third"), Some(b"literal".to_vec()));
        assert_eq!(description(b"", b"third"), None);
    }
    assert_eq!(description(b"%1", b""), Some(vec![]));
    assert_eq!(
        description(b"%1%1", b"%1\xff"),
        Some(b"%1\xff%1\xff".to_vec())
    );
}

#[test]
fn reference_edges_and_operation_order() {
    for owners in [-1, 0] {
        let mut item = Model {
            owners,
            ..Model::default()
        };
        assert!(!trigger(&mut item, true, true));
        assert_eq!(item.owners, owners);
        assert_eq!(item.trace, ["load", "release"]);
    }
    for race in [0, -1, 3] {
        let mut item = Model {
            owners: 1,
            race: Some(race),
            ..Model::default()
        };
        assert_eq!(acquire(&mut item), race > 0);
        assert_eq!(item.owners, if race > 0 { race + 1 } else { race });
    }
    let mut item = Model {
        owners: 1,
        ..Model::default()
    };
    assert!(trigger(&mut item, true, true));
    assert_eq!(item.trace, ["load", "cas", "visible", "set", "release"]);
    assert_eq!(item.owners, 1);
    item.trace.clear();
    assert!(!trigger(&mut item, false, false));
    assert_eq!(item.trace, ["load", "cas", "release"]);
}

/// The pair registry of `obs-hotkey.c`, reduced to what a rename touches:
/// each pair's two descriptions, keyed by its unchanging id, and the
/// current templates of `obs_hotkeys_set_sceneitem_hotkeys_translations()`.
/// One registered pair: its id and its two descriptions.
struct PairEntry {
    id: usize,
    show: Option<Vec<u8>>,
    hide: Option<Vec<u8>>,
}

#[derive(Default)]
struct Registry {
    show_template: Option<Vec<u8>>,
    hide_template: Option<Vec<u8>>,
    pairs: Vec<PairEntry>,
    trace: Vec<&'static str>,
}

impl Registry {
    fn translations(&mut self, show: &[u8], hide: &[u8]) {
        self.show_template = Some(show.to_vec());
        self.hide_template = Some(hide.to_vec());
    }
    /// `init_hotkeys()` (C, outside this unit) registers the pair.
    fn register(&mut self, pair: usize, show: &[u8], hide: &[u8]) {
        self.pairs.push(PairEntry {
            id: pair,
            show: Some(show.to_vec()),
            hide: Some(hide.to_vec()),
        });
    }
    fn descriptions(&self, pair: usize) -> (Option<&[u8]>, Option<&[u8]>) {
        let entry = self.pairs.iter().find(|entry| entry.id == pair).unwrap();
        (entry.show.as_deref(), entry.hide.as_deref())
    }
    /// `obs_source_set_name()` emits `rename` once per connected item.
    fn rename_source(&mut self, items: &[usize], name: &[u8]) {
        for &pair in items {
            rename(&mut Renamed {
                registry: self,
                pair,
                name: Some(name.to_vec()),
            });
        }
    }
}

struct Renamed<'a> {
    registry: &'a mut Registry,
    pair: usize,
    name: Option<Vec<u8>>,
}

impl Rename for Renamed<'_> {
    fn new_name(&mut self) -> Option<Vec<u8>> {
        self.registry.trace.push("name");
        self.name.clone()
    }
    fn template(&mut self, show: bool) -> Option<Vec<u8>> {
        self.registry.trace.push(if show { "show" } else { "hide" });
        if show {
            self.registry.show_template.clone()
        } else {
            self.registry.hide_template.clone()
        }
    }
    fn pair(&mut self) -> usize {
        self.registry.trace.push("pair");
        self.pair
    }
    fn set_descriptions(&mut self, pair: usize, show: Option<&[u8]>, hide: Option<&[u8]>) {
        self.registry.trace.push("set");
        let entry = self
            .registry
            .pairs
            .iter_mut()
            .find(|entry| entry.id == pair)
            .unwrap();
        entry.show = show.map(<[u8]>::to_vec);
        entry.hide = hide.map(<[u8]>::to_vec);
    }
}

/// `renamed_descriptions` of `test_scene_item_hotkeys.c`, 1:1 over the
/// model registry: two items of one source, their pairs 7 and 9.
#[test]
fn test_scene_item_hotkeys_renamed_registry() {
    let mut registry = Registry::default();
    registry.translations(b"Show %1", b"Hide %1");
    registry.register(7, b"Show original", b"Hide original");
    registry.register(9, b"Show original", b"Hide original");
    assert_eq!(
        registry.descriptions(7),
        (Some(&b"Show original"[..]), Some(&b"Hide original"[..]))
    );
    // New templates alone change no description.
    registry.translations(b"[%1] show %1", b"hide <%1>");
    assert_eq!(registry.descriptions(7).0, Some(&b"Show original"[..]));
    // A rename uses the current templates, replaces every placeholder and
    // updates every item of the source, under the same pair ids.
    registry.rename_source(&[7, 9], b"renamed");
    for pair in [7, 9] {
        assert_eq!(
            registry.descriptions(pair),
            (
                Some(&b"[renamed] show renamed"[..]),
                Some(&b"hide <renamed>"[..])
            )
        );
    }
    assert_eq!(
        registry
            .pairs
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
        [7, 9]
    );
    // An empty template gives a NULL description.
    registry.translations(b"literal", b"");
    registry.rename_source(&[7, 9], b"third");
    for pair in [7, 9] {
        assert_eq!(registry.descriptions(pair), (Some(&b"literal"[..]), None));
    }
}

/// The handler's reads stay in the C's order: the name, the show
/// template, the hide template, then the pair right before the setter.
#[test]
fn rename_operation_order() {
    let mut registry = Registry::default();
    registry.translations(b"Show %1", b"Hide %1");
    registry.register(3, b"", b"");
    registry.rename_source(&[3], b"x");
    assert_eq!(registry.trace, ["name", "show", "hide", "pair", "set"]);
    // A NULL name counts as "", a NULL template gives NULL.
    registry.show_template = None;
    let mut item = Renamed {
        registry: &mut registry,
        pair: 3,
        name: None,
    };
    rename(&mut item);
    assert_eq!(registry.descriptions(3), (None, Some(&b"Hide "[..])));
}
