//! Tier 1: mirrors the cull and colour part of test_raster_state_entrypoints.c
//! over a model context and a recording device, without FFI or a GPU.
use obs_graphics::raster_entrypoints::{self as raster, Entry, GS_NEITHER, Host, NOT_IN_CONTEXT};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Call {
    SetCullMode(i32),
    GetCullMode,
    EnableColor([bool; 4]),
}

#[derive(Default)]
struct Fixture {
    device: Option<usize>,
    events: Vec<(usize, Call)>,
    logs: Vec<String>,
    cull_answer: i32,
}

impl Fixture {
    fn record(&mut self, call: Call) {
        let device = self.device.expect("a device call without a context");
        self.events.push((device, call));
    }

    fn diagnostic(&mut self, entry: Entry) {
        assert_eq!(self.logs, [format!("{}: {NOT_IN_CONTEXT}", entry.name())]);
        assert!(self.events.is_empty());
        self.logs.clear();
    }

    fn expect_only(&mut self, call: Call) {
        let device = self.device.unwrap();
        assert_eq!(self.events, [(device, call)]);
        assert!(self.logs.is_empty());
        self.events.clear();
    }
}

impl Host for Fixture {
    fn has_context(&self) -> bool {
        self.device.is_some()
    }
    fn debug(&mut self, entry: Entry) {
        self.logs
            .push(format!("{}: {NOT_IN_CONTEXT}", entry.name()));
    }
    fn set_cull_mode(&mut self, mode: i32) {
        self.record(Call::SetCullMode(mode));
    }
    fn get_cull_mode(&mut self) -> i32 {
        self.record(Call::GetCullMode);
        self.cull_answer
    }
    fn enable_color(&mut self, red: bool, green: bool, blue: bool, alpha: bool) {
        self.record(Call::EnableColor([red, green, blue, alpha]));
    }
}

const GS_BACK: i32 = 0;
const GS_FRONT: i32 = 1;

#[test]
fn test_raster_state_entrypoints_missing_context() {
    // Both an absent and a released context, with the device's cull answer
    // set to something other than GS_NEITHER.
    let mut host = Fixture {
        cull_answer: GS_FRONT,
        ..Fixture::default()
    };
    for phase in 0..2 {
        if phase == 1 {
            host.device = Some(0);
            host.device = None;
        }
        raster::set_cull_mode(&mut host, GS_BACK);
        host.diagnostic(Entry::SetCullMode);
        assert_eq!(raster::get_cull_mode(&mut host), GS_NEITHER);
        host.diagnostic(Entry::GetCullMode);
        raster::enable_color(&mut host, true, false, true, false);
        host.diagnostic(Entry::EnableColor);
    }
}

fn cull_forwarding(host: &mut Fixture) {
    let modes = [GS_BACK, GS_FRONT, GS_NEITHER];
    for (m, mode) in modes.into_iter().enumerate() {
        // The numeric values are part of the ABI the port must keep.
        assert_eq!(usize::try_from(mode).unwrap(), m);
        raster::set_cull_mode(host, mode);
        host.expect_only(Call::SetCullMode(mode));
        // The getter returns the device's answer, never a cached value.
        host.cull_answer = modes[(m + 1) % 3];
        assert_eq!(raster::get_cull_mode(host), host.cull_answer);
        host.expect_only(Call::GetCullMode);
    }
    // No wrapper-side validation: undeclared values pass through both ways.
    raster::set_cull_mode(host, 77);
    host.expect_only(Call::SetCullMode(77));
    host.cull_answer = 9;
    assert_eq!(raster::get_cull_mode(host), 9);
    host.expect_only(Call::GetCullMode);
}

fn color_forwarding(host: &mut Fixture) {
    for mask in 0..16u8 {
        let rgba = [mask & 1 != 0, mask & 2 != 0, mask & 4 != 0, mask & 8 != 0];
        raster::enable_color(host, rgba[0], rgba[1], rgba[2], rgba[3]);
        // Each channel keeps its own position.
        host.expect_only(Call::EnableColor(rgba));
    }
}

#[test]
fn test_raster_state_entrypoints_forwarding() {
    for device in 0..2 {
        let mut host = Fixture {
            device: Some(device),
            cull_answer: GS_FRONT,
            ..Fixture::default()
        };
        cull_forwarding(&mut host);
        color_forwarding(&mut host);
        assert!(host.logs.is_empty());
    }
}
