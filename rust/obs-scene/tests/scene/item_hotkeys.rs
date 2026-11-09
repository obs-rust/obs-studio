use obs_scene::item_hotkeys::{Item, acquire, description, trigger};

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
