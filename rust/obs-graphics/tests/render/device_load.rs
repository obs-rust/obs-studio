//! Tier 1: every assertion of test_graphics_device_load.c, without FFI.
use obs_graphics::device_load::{self as load, Entry, Host, NOT_IN_CONTEXT};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Call {
    Vertex(Option<usize>),
    Index(Option<usize>),
    Texture(Option<usize>, i32),
    Sampler(Option<usize>, i32),
    Default(bool, i32),
}

#[derive(Default)]
struct Fixture {
    device: Option<usize>,
    calls: Vec<(usize, Call)>,
    logs: Vec<String>,
}

impl Fixture {
    fn record(&mut self, call: Call) {
        self.calls
            .push((self.device.expect("context before dispatch"), call));
    }

    fn diagnostic(&self, name: &str, count: usize) {
        assert_eq!(self.logs.len(), count);
        assert_eq!(
            self.logs.last().unwrap(),
            &format!("{name}: {NOT_IN_CONTEXT}")
        );
        assert!(self.calls.is_empty());
    }

    fn forwarded(&self, position: usize, call: Call) {
        assert_eq!(self.calls.len(), position + 1);
        assert_eq!(self.calls[position], (self.device.unwrap(), call));
        assert!(self.logs.is_empty());
    }
}

impl Host for Fixture {
    type Vertexbuffer = Option<usize>;
    type Indexbuffer = Option<usize>;
    type Texture = Option<usize>;
    type Samplerstate = Option<usize>;
    fn has_context(&self) -> bool {
        self.device.is_some()
    }
    fn debug(&mut self, entry: Entry) {
        self.logs
            .push(format!("{}: {NOT_IN_CONTEXT}", entry.name()));
    }
    fn load_vertexbuffer(&mut self, buffer: Option<usize>) {
        self.record(Call::Vertex(buffer));
    }
    fn load_indexbuffer(&mut self, buffer: Option<usize>) {
        self.record(Call::Index(buffer));
    }
    fn load_texture(&mut self, texture: Option<usize>, unit: i32) {
        self.record(Call::Texture(texture, unit));
    }
    fn load_samplerstate(&mut self, sampler: Option<usize>, unit: i32) {
        self.record(Call::Sampler(sampler, unit));
    }
    fn load_default_samplerstate(&mut self, b_3d: bool, unit: i32) {
        self.record(Call::Default(b_3d, unit));
    }
}

#[test]
fn test_graphics_device_load_no_context() {
    let mut host = Fixture::default();
    for nonnull in 0..2 {
        let pointer = (nonnull != 0).then_some(0x10);
        load::load_vertexbuffer(&mut host, pointer);
        host.diagnostic("gs_load_vertexbuffer", nonnull * 5 + 1);
        load::load_indexbuffer(&mut host, pointer);
        host.diagnostic("gs_load_indexbuffer", nonnull * 5 + 2);
        load::load_texture(&mut host, pointer, -3);
        host.diagnostic("gs_load_texture", nonnull * 5 + 3);
        load::load_samplerstate(&mut host, pointer, 7);
        host.diagnostic("gs_load_samplerstate", nonnull * 5 + 4);
        load::load_default_samplerstate(&mut host, nonnull != 0, i32::MAX);
        host.diagnostic("gs_load_default_samplerstate", nonnull * 5 + 5);
    }
}

#[test]
fn test_graphics_device_load_buffers_and_unbinds() {
    for device in [0, 1] {
        let mut host = Fixture {
            device: Some(device),
            ..Fixture::default()
        };
        load::load_vertexbuffer(&mut host, Some(0x10));
        host.forwarded(0, Call::Vertex(Some(0x10)));
        load::load_indexbuffer(&mut host, Some(0x20));
        host.forwarded(1, Call::Index(Some(0x20)));
        load::load_vertexbuffer(&mut host, None);
        host.forwarded(2, Call::Vertex(None));
        load::load_indexbuffer(&mut host, None);
        host.forwarded(3, Call::Index(None));
        host.device = None;
        host.calls.clear();
        load::load_vertexbuffer(&mut host, None);
        host.diagnostic("gs_load_vertexbuffer", 1);
    }
}

#[test]
fn test_graphics_device_load_texture_sampler_parameters() {
    for device in [0, 1] {
        let mut host = Fixture {
            device: Some(device),
            ..Fixture::default()
        };
        for unit in [0, 3, -1, i32::MIN, i32::MAX] {
            let mut n = host.calls.len();
            load::load_texture(&mut host, Some(0x30), unit);
            host.forwarded(n, Call::Texture(Some(0x30), unit));
            n += 1;
            load::load_samplerstate(&mut host, Some(0x40), unit);
            host.forwarded(n, Call::Sampler(Some(0x40), unit));
            n += 1;
            load::load_texture(&mut host, None, unit);
            host.forwarded(n, Call::Texture(None, unit));
            n += 1;
            load::load_samplerstate(&mut host, None, unit);
            host.forwarded(n, Call::Sampler(None, unit));
            n += 1;
            load::load_default_samplerstate(&mut host, false, unit);
            host.forwarded(n, Call::Default(false, unit));
            n += 1;
            load::load_default_samplerstate(&mut host, true, unit);
            host.forwarded(n, Call::Default(true, unit));
        }
    }
}

#[test]
fn entry_names_are_original_c_names() {
    for (entry, name) in [
        (Entry::Vertexbuffer, "gs_load_vertexbuffer"),
        (Entry::Indexbuffer, "gs_load_indexbuffer"),
        (Entry::Texture, "gs_load_texture"),
        (Entry::Samplerstate, "gs_load_samplerstate"),
        (Entry::DefaultSamplerstate, "gs_load_default_samplerstate"),
    ] {
        assert_eq!(entry.name(), name);
        assert_eq!(entry.c_name().to_str().unwrap(), name);
    }
}
