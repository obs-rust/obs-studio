//! Tier 1 mirrors test_graphics_planar_capability_queries.c, using fake devices.
use obs_graphics::planar_capability_queries::{Entry, Host, available};

#[derive(Default)]
struct Fake {
    context: bool,
    callbacks: [bool; 2],
    results: [bool; 2],
    device: usize,
    calls: [usize; 2],
    devices: Vec<usize>,
    logs: Vec<String>,
}

fn index(entry: Entry) -> usize {
    match entry {
        Entry::Nv12 => 0,
        Entry::P010 => 1,
    }
}

impl Host for Fake {
    fn has_context(&self) -> bool {
        self.context
    }
    fn debug(&mut self, entry: Entry) {
        self.logs.push(format!(
            "{}: called while not in a graphics context",
            entry.name()
        ));
    }
    fn has_callback(&self, entry: Entry) -> bool {
        assert!(self.context, "no projections without a context");
        self.callbacks[index(entry)]
    }
    fn dispatch(&mut self, entry: Entry) -> bool {
        assert!(self.context);
        assert!(self.callbacks[index(entry)]);
        self.calls[index(entry)] += 1;
        self.devices.push(self.device);
        self.results[index(entry)]
    }
}

#[test]
fn test_graphics_planar_capability_queries_no_context() {
    let mut host = Fake {
        callbacks: [true; 2],
        ..Fake::default()
    };
    for i in 0..2 {
        assert!(!available(&mut host, Entry::Nv12));
        assert_eq!(
            host.logs.last().unwrap(),
            "gs_nv12_available: called while not in a graphics context"
        );
        assert_eq!(host.logs.len(), 2 * i + 1);
        assert!(!available(&mut host, Entry::P010));
        assert_eq!(
            host.logs.last().unwrap(),
            "gs_p010_available: called while not in a graphics context"
        );
        assert_eq!(host.logs.len(), 2 * i + 2);
        assert_eq!(host.calls, [0, 0]);
        assert!(host.devices.is_empty());
        host.callbacks = [false; 2];
    }
}

#[test]
fn test_graphics_planar_capability_queries_optional_callbacks_are_independent() {
    let mut host = Fake {
        context: true,
        callbacks: [false, true],
        results: [true; 2],
        ..Fake::default()
    };
    assert!(!available(&mut host, Entry::Nv12));
    assert!(available(&mut host, Entry::P010));
    assert_eq!(host.calls, [0, 1]);
    host.callbacks = [true, false];
    assert!(!available(&mut host, Entry::P010));
    assert!(available(&mut host, Entry::Nv12));
    assert_eq!(host.calls, [1, 1]);
    host.callbacks = [false; 2];
    assert!(!available(&mut host, Entry::Nv12));
    assert!(!available(&mut host, Entry::P010));
    assert_eq!(host.calls, [1, 1]);
    assert!(host.logs.is_empty());
}

#[test]
fn test_graphics_planar_capability_queries_callback_results_and_device_routing() {
    let mut host = Fake {
        callbacks: [true; 2],
        ..Fake::default()
    };
    for device in 0..2 {
        host.device = device;
        host.context = true;
        for results in 0..4 {
            host.results = [results & 1 != 0, results & 2 != 0];
            let calls = device * 4 + results;
            assert_eq!(available(&mut host, Entry::Nv12), host.results[0]);
            assert_eq!(host.calls, [calls + 1, calls]);
            assert_eq!(available(&mut host, Entry::P010), host.results[1]);
            assert_eq!(host.calls, [calls + 1, calls + 1]);
            assert_eq!(&host.devices[2 * calls..], &[device, device]);
            assert!(host.logs.is_empty());
        }
        host.context = false;
        assert!(!host.has_context());
    }
}
