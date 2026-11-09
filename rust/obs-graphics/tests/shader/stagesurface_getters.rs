use obs_graphics::stagesurface_getters::{Host, Property, metadata};
#[derive(Default)]
struct Fixture {
    context: bool,
    calls: [u32; 3],
    seen: Option<usize>,
    values: [u32; 3],
    logs: Vec<(&'static str, &'static str)>,
    references: u32,
}
impl Host<usize> for Fixture {
    fn has_context(&self) -> bool { self.context }
    fn debug(&mut self, function: &'static str, reason: &'static str) { self.logs.push((function, reason)); }
    fn dispatch(&mut self, property: Property, surface: usize) -> u32 {
        let i = property as usize;
        self.calls[i] += 1;
        self.seen = Some(surface);
        self.values[i]
    }
}
/// Mirrors every assertion of test_graphics_stagesurface_getters.c.
#[test]
fn test_graphics_stagesurface_getters() {
    let properties = [Property::Width, Property::Height, Property::ColorFormat];
    let mut host = Fixture::default();
    let surface = 0x1234;
    let mut count = 0;
    for token in [None, Some(surface)] {
        for property in properties {
            assert_eq!(metadata(&mut host, token, property), 0);
            count += 1;
            assert_eq!(host.logs.len(), count);
            assert_eq!(host.logs.last(), Some(&(property.function(), "called while not in a graphics context")));
        }
    }
    host.context = true;
    host.references = 1;
    for property in properties {
        assert_eq!(metadata(&mut host, None, property), 0);
        count += 1;
        assert_eq!(host.logs.len(), count);
        assert_eq!(host.logs.last(), Some(&(property.function(), "Null 'stagesurf' parameter")));
    }
    assert_eq!(host.calls, [0; 3]);
    assert_eq!(host.seen, None);
    let sizes = [0, 1, 1920, u32::MAX];
    let formats = [0, 3, 8, 1];
    for i in 0..4 {
        host.values = [sizes[i], sizes[3-i], formats[i]];
        for (j, property) in properties.into_iter().enumerate() {
            host.seen = None;
            let expected = host.values[j];
            assert_eq!(metadata(&mut host, Some(surface), property), expected);
            assert_eq!(host.seen, Some(surface));
            for k in 0..3 { assert_eq!(host.calls[k], (i + usize::from(k <= j) ) as u32); }
        }
        assert!(host.context);
        assert_eq!(host.references, 1);
        assert_eq!(host.logs.len(), 9);
    }
    host.context = false;
    host.references = 0;
    for property in properties {
        assert_eq!(metadata(&mut host, Some(surface), property), 0);
        count += 1;
        assert_eq!(host.logs.len(), count);
        assert_eq!(host.logs.last(), Some(&(property.function(), "called while not in a graphics context")));
    }
    assert!(!host.context);
    assert_eq!(host.references, 0);
    assert_eq!(host.calls, [4; 3]);
}
