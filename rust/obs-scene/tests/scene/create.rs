//! Tier 1 counterpart of test_scene_create.c: routing preserves the host contract.
use obs_scene::create::{Host, create};

#[derive(Default)]
struct SourceHost {
    calls: Vec<(usize, &'static [u8; 6], Option<&'static str>, bool)>,
    notifications: Vec<usize>,
    private: bool,
    name: Option<String>,
    references: usize,
}
impl SourceHost {
    fn source(
        &mut self,
        canvas: usize,
        id: &'static [u8; 6],
        name: Option<&'static str>,
        private: bool,
    ) -> usize {
        self.calls.push((canvas, id, name, private));
        self.private = private;
        self.name = match name {
            None if private => None,
            Some(n) if !n.is_empty() => Some(n.into()),
            _ => Some("__unnamed0001".into()),
        };
        self.references = if private { 1 } else { 2 };
        if !private {
            self.notifications.extend([73, 73]);
        }
        73
    }
}
impl Host for SourceHost {
    type Handle = usize;
    type Name = Option<&'static str>;
    fn main_canvas(&self) -> usize {
        41
    }
    fn public(&mut self, canvas: usize, id: &'static [u8; 6], name: Self::Name) -> usize {
        self.source(canvas, id, name, false)
    }
    fn private(&mut self, canvas: usize, id: &'static [u8; 6], name: Self::Name) -> usize {
        self.source(canvas, id, name, true)
    }
}

#[test]
fn test_scene_create_public_names() {
    characterize(false);
}
#[test]
fn test_scene_create_private_names() {
    characterize(true);
}
fn characterize(private: bool) {
    for name in [Some("constructor name"), Some(""), None] {
        let mut host = SourceHost::default();
        let scene = create(&mut host, name, private);
        assert_eq!(scene, 73); // Exact context-data identity, not the source handle.
        assert_eq!(host.calls, [(41, b"scene\0", name, private)]);
        assert_eq!(host.private, private);
        if private && name.is_none() {
            assert_eq!(host.name, None);
        } else if let Some(n) = name.filter(|n| !n.is_empty()) {
            assert_eq!(host.name.as_deref(), Some(n));
        } else {
            let generated = host.name.as_deref().unwrap();
            assert!(generated.starts_with("__unnamed") && generated.len() >= 13);
        }
        assert_eq!(
            host.notifications,
            if private { vec![] } else { vec![scene, scene] }
        );
        assert_eq!(host.references, if private { 1 } else { 2 });
        // Source-owned settings, signal registrations and type are delegated,
        // not reimplemented by this two-call routing core. Real instances and
        // all lifecycle assertions remain covered by the unchanged Tier 2 test.
    }
}

#[test]
fn passes_through_host_result() {
    struct Empty;
    impl Host for Empty {
        type Handle = usize;
        type Name = ();
        fn main_canvas(&self) -> usize {
            0
        }
        fn public(&mut self, c: usize, id: &'static [u8; 6], _: ()) -> usize {
            assert_eq!((c, id), (0, b"scene\0"));
            0
        }
        fn private(&mut self, c: usize, id: &'static [u8; 6], n: ()) -> usize {
            self.public(c, id, n)
        }
    }
    assert_eq!(create(&mut Empty, (), false), 0);
    assert_eq!(create(&mut Empty, (), true), 0);
}
