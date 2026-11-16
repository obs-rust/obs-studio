//! Safe counterparts of every assertion in test_voltexture_metadata.c.
use obs_graphics::voltexture_metadata::{Host, Property, metadata};

#[derive(Default)]
struct Fixture {
    context: bool,
    calls: [u32; 4],
    seen: Option<usize>,
    values: [u32; 4],
    logs: Vec<(&'static str, &'static str)>,
}
impl Host<usize> for Fixture {
    fn has_context(&self) -> bool {
        self.context
    }
    fn debug(&mut self, function: &'static str, reason: &'static str) {
        self.logs.push((function, reason));
    }
    fn dispatch(&mut self, property: Property, texture: usize) -> u32 {
        let i = property as usize;
        self.calls[i] += 1;
        self.seen = Some(texture);
        self.values[i]
    }
}
const PROPERTIES: [Property; 4] = [
    Property::Width,
    Property::Height,
    Property::Depth,
    Property::ColorFormat,
];

#[test]
fn test_voltexture_metadata_rejected_inputs() {
    let mut host = Fixture::default();
    for active in [false, true] {
        host.context = active;
        for token in [None, Some(0x1234)] {
            if active && token.is_some() {
                continue;
            }
            for property in PROPERTIES {
                assert_eq!(metadata(&mut host, token, property), 0);
                assert_eq!(host.logs.len(), 1);
                assert_eq!(
                    host.logs[0],
                    (
                        property.function(),
                        if active {
                            "Null 'voltex' parameter"
                        } else {
                            "called while not in a graphics context"
                        }
                    )
                );
                assert_eq!(host.calls, [0; 4]);
                assert_eq!(host.seen, None);
                host.logs.clear();
            }
        }
    }
}

#[test]
fn test_voltexture_metadata_backend_results() {
    let mut host = Fixture {
        context: true,
        ..Fixture::default()
    };
    let values = [0, 1, 1920, u32::MAX];
    let colors = [0, 3, 2, 9]; // GS_UNKNOWN, GS_RGBA, GS_R8, GS_RGBA16F
    for i in 0..4 {
        host.values = [values[i], values[3 - i], values[(i + 1) % 4], colors[i]];
        for (j, property) in PROPERTIES.into_iter().enumerate() {
            let expected = host.values[j];
            assert_eq!(metadata(&mut host, Some(0x1234), property), expected);
            assert_eq!(host.seen, Some(0x1234));
            assert_eq!(
                host.calls,
                std::array::from_fn(|k| (i + usize::from(k <= j)) as u32)
            );
            assert!(host.logs.is_empty());
        }
    }
}

#[test]
fn test_voltexture_metadata_all_formats() {
    let mut host = Fixture {
        context: true,
        ..Fixture::default()
    };
    for format in 0..23 {
        host.values[3] = format;
        assert_eq!(
            metadata(&mut host, Some(0x1234), Property::ColorFormat),
            format
        );
        assert_eq!(host.calls, [0, 0, 0, format + 1]);
        assert_eq!(host.seen, Some(0x1234));
        assert!(host.logs.is_empty());
    }
}
