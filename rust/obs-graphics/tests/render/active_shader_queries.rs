//! Tier 1 mirrors the frozen test_graphics_active_shader_queries.c assertions.
use obs_graphics::active_shader_queries::{Entry, Host, shader};

#[derive(Default)]
struct Fake {
    context: bool,
    callbacks: [bool; 2],
    results: [Option<usize>; 2],
    device: usize,
    calls: [usize; 2],
    devices: Vec<usize>,
    logs: Vec<String>,
    reference_count: usize,
    tokens: [i32; 2],
}

impl Fake {
    fn enter(&mut self, device: Option<usize>) {
        if let Some(device) = device {
            self.context = true;
            self.device = device;
            self.reference_count = 1;
        } else {
            self.logs
                .push("gs_enter_context: Null 'graphics' parameter".into());
        }
    }

    fn leave(&mut self) {
        self.context = false;
        self.reference_count = 0;
    }
}

impl Host for Fake {
    type Shader = Option<usize>;
    fn has_context(&self) -> bool {
        self.context
    }
    fn debug(&mut self, entry: Entry) {
        self.logs.push(format!(
            "{}: called while not in a graphics context",
            entry.name()
        ));
    }
    fn dispatch(&mut self, entry: Entry) -> Self::Shader {
        assert!(self.context, "NULL checked before any projection");
        let index = match entry {
            Entry::Vertex => 0,
            Entry::Pixel => 1,
        };
        assert!(self.callbacks[index], "mandatory callback");
        self.calls[index] += 1;
        self.devices.push(self.device);
        self.results[index]
    }
}

#[test]
fn test_graphics_active_shader_queries_no_context() {
    let mut host = Fake {
        callbacks: [true; 2],
        ..Fake::default()
    };
    host.enter(None);
    assert_eq!(host.logs[0], "gs_enter_context: Null 'graphics' parameter");
    assert_eq!(host.logs.len(), 1);
    assert!(!host.has_context());
    for i in 0..2 {
        assert_eq!(shader(&mut host, Entry::Vertex), None);
        assert_eq!(
            host.logs.last().unwrap(),
            "gs_get_vertex_shader: called while not in a graphics context"
        );
        assert_eq!(host.logs.len(), 2 * i + 2);
        assert_eq!(shader(&mut host, Entry::Pixel), None);
        assert_eq!(
            host.logs.last().unwrap(),
            "gs_get_pixel_shader: called while not in a graphics context"
        );
        assert_eq!(host.logs.len(), 2 * i + 3);
        assert_eq!(host.calls, [0; 2]);
        assert!(host.devices.is_empty());
        host.callbacks = [false; 2];
    }
}

#[test]
fn test_graphics_active_shader_queries_callback_results_and_device_routing() {
    let mut host = Fake {
        callbacks: [true; 2],
        tokens: [17, 29],
        ..Fake::default()
    };
    for device in 0..2 {
        host.enter(Some(device));
        for results in 0..4 {
            host.results = [
                (results & 1 != 0).then_some(device),
                (results & 2 != 0).then_some(1 - device),
            ];
            let calls = device * 4 + results;
            assert_eq!(shader(&mut host, Entry::Vertex), host.results[0]);
            assert_eq!(host.calls, [calls + 1, calls]);
            assert_eq!(shader(&mut host, Entry::Pixel), host.results[1]);
            assert_eq!(host.calls, [calls + 1, calls + 1]);
            assert_eq!(&host.devices[2 * calls..], &[device, device]);
            assert_eq!(host.reference_count, 1);
            assert_eq!(host.tokens, [17, 29]);
            assert!(host.logs.is_empty());
        }
        host.leave();
        assert!(!host.has_context());
    }
}

#[test]
fn test_graphics_active_shader_queries_leaving_context_rejects_then_reentry_dispatches() {
    let mut host = Fake {
        callbacks: [true; 2],
        results: [Some(0), Some(1)],
        ..Fake::default()
    };
    host.enter(Some(0));
    assert_eq!(shader(&mut host, Entry::Pixel), Some(1));
    assert_eq!(shader(&mut host, Entry::Vertex), Some(0));
    host.leave();
    assert_eq!(shader(&mut host, Entry::Vertex), None);
    assert_eq!(
        host.logs.last().unwrap(),
        "gs_get_vertex_shader: called while not in a graphics context"
    );
    assert_eq!(shader(&mut host, Entry::Pixel), None);
    assert_eq!(
        host.logs.last().unwrap(),
        "gs_get_pixel_shader: called while not in a graphics context"
    );
    assert_eq!(host.logs.len(), 2);
    assert_eq!(host.calls, [1; 2]);
    host.enter(Some(0));
    assert_eq!(shader(&mut host, Entry::Vertex), Some(0));
    assert_eq!(shader(&mut host, Entry::Pixel), Some(1));
    assert_eq!(host.calls, [2; 2]);
    assert_eq!(host.logs.len(), 2);
}
