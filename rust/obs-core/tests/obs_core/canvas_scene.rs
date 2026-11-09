use obs_core::canvas_scene::{self, Host};

#[derive(Default)]
struct CanvasHost {
    scenes: Vec<(usize, &'static str, Option<usize>)>,
    calls: Vec<&'static str>,
}
impl Host for CanvasHost {
    type Handle = usize;
    type Name = &'static str;
    fn create_scene_source(&mut self, canvas: usize, name: Self::Name) -> usize {
        self.calls.push("create");
        self.scenes.push((self.scenes.len(), name, Some(canvas)));
        self.scenes.len() - 1
    }
    fn source_scene(&self, source: usize) -> usize { self.scenes[source].0 }
    fn scene_source(&self, scene: usize) -> usize { scene }
    fn remove_source(&mut self, source: usize) {
        self.calls.push("remove");
        self.scenes[source].2 = None;
    }
}
impl CanvasHost {
    fn find(&self, canvas: usize, name: &str) -> Option<usize> {
        self.scenes.iter().find(|(_, n, c)| *n == name && *c == Some(canvas)).map(|s| s.0)
    }
}

// Mirrors test_scene_canvas_characterization.c:canvas_scene_remove_and_move.
// Canvas allocation, lookup and move stay host operations outside this swap.
#[test]
fn canvas_scene_remove_and_move() {
    let mut host = CanvasHost::default();
    let (src, dst) = (1, 2);
    let moved = canvas_scene::create(&mut host, src, "char moved scene");
    let removed = canvas_scene::create(&mut host, src, "char removed scene");
    assert_ne!(moved, removed);
    assert_eq!(host.scenes[moved].2, Some(src));
    assert_eq!(host.find(src, "char moved scene"), Some(moved));
    host.scenes[moved].2 = Some(dst);
    assert_eq!(host.find(src, "char moved scene"), None);
    assert_eq!(host.find(dst, "char moved scene"), Some(moved));
    assert_eq!(host.scenes[moved].2, Some(dst));
    canvas_scene::remove(&mut host, removed);
    assert_eq!(host.find(src, "char removed scene"), None);
    assert_eq!(host.scenes[removed].2, None);
    assert_eq!(host.scenes[moved].2, Some(dst));
    assert_eq!(host.calls, ["create", "create", "remove"]);
}

#[test]
fn removal_is_delegated_even_when_already_unowned() {
    let mut host = CanvasHost::default();
    let scene = canvas_scene::create(&mut host, 0, "");
    canvas_scene::remove(&mut host, scene);
    canvas_scene::remove(&mut host, scene);
    assert_eq!(host.scenes[scene].2, None);
    assert_eq!(host.calls, ["create", "remove", "remove"]);
}
