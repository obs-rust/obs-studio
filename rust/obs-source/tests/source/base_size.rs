//! Mirrors test_source_base_size.c through the safe API.
use obs_source::base_size::{self, Axis, Source};
use obs_source::size_route::Node;
use std::cell::Cell;

#[derive(Clone, Copy)]
struct Fixture<'a> {
    node: Node,
    callback: Option<&'a Cell<u32>>,
    transition: [u32; 2],
    asynchronous: [u32; 2],
    target: Option<&'a Fixture<'a>>,
}
impl Source for Fixture<'_> {
    fn has_data(self) -> bool {
        self.node.has_data
    }
    fn node(self, _: Axis) -> Node {
        Node {
            has_callback: self.callback.is_some(),
            ..self.node
        }
    }
    fn transition_size(self, axis: Axis) -> u32 {
        self.transition[usize::from(axis == Axis::Height)]
    }
    fn callback_size(self, axis: Axis) -> u32 {
        self.callback
            .unwrap()
            .get()
            .wrapping_add(u32::from(axis == Axis::Height))
    }
    fn target(self) -> Option<Self> {
        self.target.copied()
    }
    fn async_size(self, axis: Axis) -> u32 {
        self.asynchronous[usize::from(axis == Axis::Height)]
    }
}
fn sizes(source: Option<Fixture<'_>>) -> [u32; 2] {
    [Axis::Width, Axis::Height].map(|axis| base_size::query(source, axis, |_| {}))
}
#[test]
fn source_base_size_characterization() {
    assert_eq!(sizes(None), [0, 0]);
    let value = Cell::new(123);
    let mut source = Fixture {
        node: Node {
            has_data: true,
            ..Node::default()
        },
        callback: None,
        transition: [640, 480],
        asynchronous: [1920, 1080],
        target: None,
    };
    assert_eq!(sizes(Some(source)), [0, 0]);
    source.callback = Some(&value);
    assert_eq!(sizes(Some(source)), [123, 124]);
    value.set(u32::MAX);
    assert_eq!(sizes(Some(source)), [u32::MAX, 0]);
    source.node.source_type = 2;
    source.node.enabled = true;
    assert_eq!(sizes(Some(source)), [640, 480]);
    source.node.source_type = 0;
    source.node.has_data = false;
    assert_eq!(sizes(Some(source)), [0, 0]);
    source.node.has_data = true;
    source.callback = None;
    source.node.async_active = true;
    for angle in (-360..=360).step_by(90) {
        source.node.async_rotation = angle;
        assert_eq!(
            sizes(Some(source)),
            if angle % 180 == 0 {
                [1920, 1080]
            } else {
                [1080, 1920]
            }
        );
    }
    let mut filter = Fixture {
        node: Node {
            source_type: 1,
            has_data: true,
            has_filter_parent: true,
            ..Node::default()
        },
        ..source
    };
    assert_eq!(sizes(Some(filter)), [0, 0]);
    source.node.async_rotation = 0;
    filter.target = Some(&source);
    assert_eq!(sizes(Some(filter)), [1920, 1080]);
    let inactive = Fixture {
        node: Node {
            async_active: false,
            ..source.node
        },
        ..source
    };
    filter.target = Some(&inactive);
    assert_eq!(sizes(Some(filter)), [0, 0]);
}
#[test]
fn only_public_root_requires_context_data() {
    let source = Fixture {
        node: Node {
            async_active: true,
            ..Node::default()
        },
        callback: None,
        transition: [0, 0],
        asynchronous: [7, 9],
        target: None,
    };
    assert_eq!(sizes(Some(source)), [0, 0]);
    assert_eq!(base_size::base(source, Axis::Width), 7);
    let mut logged = Vec::new();
    for axis in [Axis::Width, Axis::Height] {
        assert_eq!(
            base_size::query::<Fixture<'_>>(None, axis, |a| logged.push(a)),
            0
        );
    }
    assert_eq!(logged, [Axis::Width, Axis::Height]);
}
