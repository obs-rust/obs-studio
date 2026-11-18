//! Tier 1 mirrors test_graphics_target_size.c using safe output-slot handles.
use obs_graphics::target_size::{self as queries, Entry, Host};

struct Fixture {
    context: bool,
    callbacks: bool,
    values: [u32; 2],
    outputs: [u32; 2],
    expected: [Option<usize>; 2],
    calls: [u32; 3],
    logs: Vec<(i32, String)>,
}
impl Default for Fixture {
    fn default() -> Self {
        Self {
            context: false,
            callbacks: true,
            values: [1920, 1080],
            outputs: [0xa5a5_a5a5, 0x5a5a_5a5a],
            expected: [None, None],
            calls: [0; 3],
            logs: Vec::new(),
        }
    }
}
impl Host for Fixture {
    type Output = Option<usize>;
    fn has_context(&self) -> bool {
        self.context
    }
    fn debug(&mut self, entry: Entry) {
        self.logs.push((
            400,
            format!("{}: called while not in a graphics context", entry.name()),
        ));
    }
    fn size(&mut self, x: Self::Output, y: Self::Output) {
        assert!(self.context && self.callbacks);
        assert_eq!([x, y], self.expected);
        self.calls[0] += 1;
        if let Some(x) = x {
            self.outputs[x] = self.values[0];
        }
        if let Some(y) = y {
            self.outputs[y] = self.values[1];
        }
    }
    fn width(&mut self) -> u32 {
        assert!(self.context && self.callbacks);
        self.calls[1] += 1;
        self.values[0]
    }
    fn height(&mut self) -> u32 {
        assert!(self.context && self.callbacks);
        self.calls[2] += 1;
        self.values[1]
    }
}
fn diagnostic(host: &Fixture, entry: Entry, count: usize) {
    assert_eq!(host.logs.len(), count);
    assert_eq!(
        host.logs.last(),
        Some(&(
            400,
            format!("{}: called while not in a graphics context", entry.name())
        ))
    );
}
#[test]
fn test_graphics_target_size_no_context() {
    let mut host = Fixture::default();
    for (i, callbacks) in [true, false].into_iter().enumerate() {
        host.callbacks = callbacks;
        queries::get_size(&mut host, Some(0), Some(1));
        diagnostic(&host, Entry::Size, 4 * i + 1);
        assert_eq!(host.outputs, [0xa5a5_a5a5, 0x5a5a_5a5a]);
        queries::get_size(&mut host, None, None);
        diagnostic(&host, Entry::Size, 4 * i + 2);
        assert_eq!(queries::get_width(&mut host), 0);
        diagnostic(&host, Entry::Width, 4 * i + 3);
        assert_eq!(queries::get_height(&mut host), 0);
        diagnostic(&host, Entry::Height, 4 * i + 4);
        assert_eq!(host.calls, [0; 3]);
    }
}
#[test]
fn test_graphics_target_size_size_values() {
    let mut host = Fixture {
        context: true,
        expected: [Some(0), Some(1)],
        ..Fixture::default()
    };
    for (i, values) in [
        [1920, 1080],
        [0, 0],
        [0x8000_0000, u32::MAX],
        [u32::MAX, 0x8000_0000],
    ]
    .into_iter()
    .enumerate()
    {
        host.values = values;
        host.outputs = [17; 2];
        queries::get_size(&mut host, Some(0), Some(1));
        assert_eq!(host.outputs, values);
        assert_eq!(host.calls, [i as u32 + 1, 0, 0]);
        assert!(host.logs.is_empty());
    }
}
#[test]
fn test_graphics_target_size_nullable_and_aliased_outputs() {
    let mut host = Fixture {
        context: true,
        outputs: [17; 2],
        ..Fixture::default()
    };
    for (i, (outputs, value)) in [
        ([None, None], 17),
        ([Some(0), None], 1920),
        ([None, Some(0)], 1080),
        ([Some(0), Some(0)], 1080),
    ]
    .into_iter()
    .enumerate()
    {
        host.expected = outputs;
        queries::get_size(&mut host, outputs[0], outputs[1]);
        assert_eq!(host.outputs[0], value);
        assert_eq!(host.outputs[1], 17);
        assert_eq!(host.calls, [i as u32 + 1, 0, 0]);
        assert!(host.logs.is_empty());
    }
}
#[test]
fn test_graphics_target_size_scalar_values() {
    let mut host = Fixture {
        context: true,
        ..Fixture::default()
    };
    let values = [1920, 1080, 0, 1, 0x8000_0000, u32::MAX];
    for (i, value) in values.into_iter().enumerate() {
        host.values = [value, values[values.len() - i - 1]];
        assert_eq!(queries::get_width(&mut host), host.values[0]);
        assert_eq!(host.calls, [0, i as u32 + 1, i as u32]);
        assert_eq!(queries::get_height(&mut host), host.values[1]);
        assert_eq!(host.calls, [0, i as u32 + 1, i as u32 + 1]);
        assert!(host.logs.is_empty());
    }
}
#[test]
fn diagnostic_names_match_c() {
    for entry in [Entry::Size, Entry::Width, Entry::Height] {
        assert_eq!(entry.c_name().to_bytes(), entry.name().as_bytes());
    }
}
