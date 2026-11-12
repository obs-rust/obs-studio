//! Tier 1: assertions from test_canvas_move_scene.c, using safe host identities.
use obs_core::canvas_move::{Host, move_scene};

#[derive(Debug, PartialEq)]
struct Event(usize, usize, bool);

struct Model {
    owners: [Option<usize>; 5],
    items: [Vec<usize>; 3],
    events: Vec<Event>,
    scene_refs: [bool; 2],
}

impl Model {
    fn new(scene_refs: [bool; 2]) -> Self {
        Self {
            owners: [Some(0); 5],
            items: [vec![], vec![], vec![]],
            events: vec![],
            scene_refs,
        }
    }
    fn find(&self, canvas: usize, source: usize) -> Option<usize> {
        (self.owners[source] == Some(canvas)).then_some(source)
    }
}

impl Host for Model {
    type Handle = usize;
    fn scene_source(&self, scene: usize) -> usize { scene }
    fn item_source(&self, item: usize) -> usize { item }
    fn is_group(&self, source: usize) -> bool { source >= 3 }
    fn remove_source(&mut self, source: usize) {
        if let Some(owner) = self.owners[source].take() {
            self.events.push(Event(source, owner, false));
            // Existing remove_source detaches shared group items from scenes
            // still on the old canvas, including a same-canvas moved scene.
            for scene in 0..3 {
                if self.owners[scene] == Some(owner) {
                    self.items[scene].retain(|&item| item != source);
                }
            }
        }
    }
    fn insert_source(&mut self, canvas: usize, source: usize) {
        self.owners[source] = Some(canvas);
        self.events.push(Event(source, canvas, true));
    }
    fn enumerate_items<F: FnMut(&mut Self, usize)>(&mut self, scene: usize, mut visit: F) {
        // The existing enumerator retains each item, so removing its source
        // during the callback must not invalidate this visit.
        for item in self.items[scene].clone() { visit(self, item); }
    }
}

#[test]
fn empty_scene_move() {
    for flags in [[false, true], [true, false]] {
        let mut host = Model::new(flags);
        assert_eq!(host.scene_refs, flags);
        move_scene(&mut host, 0, 1);
        assert_eq!(host.owners[0], Some(1));
        assert_eq!(host.find(0, 0), None);
        assert_eq!(host.find(1, 0), Some(0));
        assert_eq!(host.items[0].len(), 0);
        move_scene(&mut host, 0, 0);
        assert_eq!(host.owners[0], Some(0));
        assert_eq!(host.find(1, 0), None);
    }
}

fn migrate_groups(same_canvas: bool) {
    let dst = usize::from(!same_canvas);
    let mut host = Model::new([false; 2]);
    // Scene 0, observer 1, ordinary child 2, groups 3 and 4.
    // Deliberately reverse creation order, with the ordinary source between.
    host.items = [vec![4, 2, 3], vec![3, 4], vec![2]];
    move_scene(&mut host, 0, dst);
    assert_eq!(host.events.len(), 6);
    for (i, source) in [0, 4, 3].into_iter().enumerate() {
        assert_eq!(host.events[2 * i], Event(source, 0, false));
        assert_eq!(host.events[2 * i + 1], Event(source, dst, true));
        assert_eq!(host.owners[source], Some(dst));
    }
    assert_eq!(host.owners[2], Some(0));
    assert_eq!(host.owners[1], Some(0));
    assert_eq!(host.items[0].len(), if same_canvas { 1 } else { 3 });
    assert_eq!(host.items[2].len(), 1); // ordinary child inside first group
    assert!(host.items[1].is_empty());
    for source in [3, 4] {
        assert_eq!(host.items[0].contains(&source), !same_canvas);
        if !same_canvas { assert_eq!(host.find(0, source), None); }
    }
    assert_eq!(host.find(dst, 3), Some(3));
}

#[test]
fn group_migration_order() { migrate_groups(false); }

#[test]
fn same_canvas_migration() { migrate_groups(true); }
