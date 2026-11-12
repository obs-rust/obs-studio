//! Tier 1: mirrors test_present_entrypoints.c over a model context and a
//! recording device, without FFI or a GPU.
use obs_graphics::present_entrypoints::{self as present, Entry, Host, NOT_IN_CONTEXT};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Call {
    BeginFrame,
    BeginScene,
    Draw(i32, u32, u32),
    EndScene,
    LoadSwapchain(Option<usize>),
    Clear(u32, Option<usize>, u32, u8),
    IsPresentReady,
    Present,
    Flush,
}

#[derive(Default)]
struct Fixture {
    device: Option<usize>,
    events: Vec<(usize, Call)>,
    logs: Vec<String>,
    present_ready: bool,
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
    type Swapchain = Option<usize>;
    type Color = Option<usize>;

    fn has_context(&self) -> bool {
        self.device.is_some()
    }
    fn debug(&mut self, entry: Entry) {
        self.logs
            .push(format!("{}: {NOT_IN_CONTEXT}", entry.name()));
    }
    fn begin_frame(&mut self) {
        self.record(Call::BeginFrame);
    }
    fn begin_scene(&mut self) {
        self.record(Call::BeginScene);
    }
    fn draw(&mut self, mode: i32, start_vert: u32, num_verts: u32) {
        self.record(Call::Draw(mode, start_vert, num_verts));
    }
    fn end_scene(&mut self) {
        self.record(Call::EndScene);
    }
    fn load_swapchain(&mut self, swapchain: Option<usize>) {
        self.record(Call::LoadSwapchain(swapchain));
    }
    fn clear(&mut self, flags: u32, color: Option<usize>, depth: f32, stencil: u8) {
        self.record(Call::Clear(flags, color, depth.to_bits(), stencil));
    }
    fn is_present_ready(&mut self) -> bool {
        self.record(Call::IsPresentReady);
        self.present_ready
    }
    fn present(&mut self) {
        self.record(Call::Present);
    }
    fn flush(&mut self) {
        self.record(Call::Flush);
    }
}

const CLEAR_COLOR: u32 = 1 << 0;
const CLEAR_DEPTH: u32 = 1 << 1;
const CLEAR_STENCIL: u32 = 1 << 2;
const TRIS: i32 = 3;
const TRISTRIP: i32 = 4;
const COLOR: Option<usize> = Some(0x10);

#[test]
fn test_present_entrypoints_missing_context() {
    // Both an absent and a released context; the device would answer true.
    let mut host = Fixture {
        present_ready: true,
        ..Fixture::default()
    };
    for phase in 0..2 {
        if phase == 1 {
            host.device = Some(0);
            host.device = None;
        }
        present::begin_frame(&mut host);
        host.diagnostic(Entry::BeginFrame);
        present::begin_scene(&mut host);
        host.diagnostic(Entry::BeginScene);
        present::draw(&mut host, TRIS, 3, 6);
        host.diagnostic(Entry::Draw);
        present::end_scene(&mut host);
        host.diagnostic(Entry::EndScene);
        present::load_swapchain(&mut host, Some(0x20));
        host.diagnostic(Entry::LoadSwapchain);
        present::load_swapchain(&mut host, None);
        host.diagnostic(Entry::LoadSwapchain);
        present::clear(
            &mut host,
            CLEAR_COLOR | CLEAR_DEPTH | CLEAR_STENCIL,
            COLOR,
            1.0,
            0,
        );
        host.diagnostic(Entry::Clear);
        present::clear(&mut host, 0, None, 0.0, 0);
        host.diagnostic(Entry::Clear);
        assert!(!present::is_present_ready(&mut host));
        host.diagnostic(Entry::IsPresentReady);
        present::present(&mut host);
        host.diagnostic(Entry::Present);
        present::flush(&mut host);
        host.diagnostic(Entry::Flush);
    }
}

fn draw_forwarding(host: &mut Fixture) {
    let ranges = [(0, 0), (0, 3), (7, 1), (u32::MAX, u32::MAX), (1, u32::MAX)];
    // GS_POINTS .. GS_TRISTRIP are 0..=4.
    for mode in 0..=TRISTRIP {
        for (start, count) in ranges {
            present::draw(host, mode, start, count);
            host.expect_only(Call::Draw(mode, start, count));
        }
    }
    // No check of the mode: an undeclared value passes through.
    present::draw(host, 99, 2, 4);
    host.expect_only(Call::Draw(99, 2, 4));
}

fn clear_forwarding(host: &mut Fixture) {
    let flags = [
        0,
        CLEAR_COLOR,
        CLEAR_DEPTH,
        CLEAR_STENCIL,
        CLEAR_COLOR | CLEAR_DEPTH | CLEAR_STENCIL,
        0xFFFF_FFF8,
        u32::MAX,
    ];
    let depths = [
        0.0f32.to_bits(),
        0x8000_0000,
        1.0f32.to_bits(),
        (-2.5f32).to_bits(),
        0x7F80_0000,
        0xFF80_0000,
        0x7FC0_0001,
        0x0000_0001,
    ];
    let stencils = [0, 1, 0x80, 0xFF];
    let colors = [COLOR, None];
    let mut n = 0;
    for flag in flags {
        for depth in depths {
            let color = colors[n % 2];
            let stencil = stencils[n % stencils.len()];
            n += 1;
            present::clear(host, flag, color, f32::from_bits(depth), stencil);
            // The colour by identity; the depth's exact bits (signed zero,
            // infinities and a NaN payload).
            host.expect_only(Call::Clear(flag, color, depth, stencil));
        }
    }
}

#[test]
fn test_present_entrypoints_forwarding() {
    let mut host = Fixture::default();
    for device in 0..2 {
        host.device = Some(device);

        present::begin_frame(&mut host);
        host.expect_only(Call::BeginFrame);
        present::begin_scene(&mut host);
        host.expect_only(Call::BeginScene);
        present::end_scene(&mut host);
        host.expect_only(Call::EndScene);
        present::present(&mut host);
        host.expect_only(Call::Present);
        present::flush(&mut host);
        host.expect_only(Call::Flush);

        // NULL is forwarded too: the swapchain is not checked.
        for swapchain in [Some(0x20), Some(0x21), None] {
            present::load_swapchain(&mut host, swapchain);
            host.expect_only(Call::LoadSwapchain(swapchain));
        }

        // The device's answer, as it is, each time it is asked.
        for answer in [true, false, true] {
            host.present_ready = answer;
            assert_eq!(present::is_present_ready(&mut host), answer);
            host.expect_only(Call::IsPresentReady);
        }

        draw_forwarding(&mut host);
        clear_forwarding(&mut host);

        host.device = None;
    }
}

#[test]
fn test_present_entrypoints_frame_sequence() {
    // One device call per call, in call order, with no state of its own:
    // repeated and out-of-order calls are all forwarded.
    let mut host = Fixture {
        device: Some(0),
        present_ready: true,
        ..Fixture::default()
    };
    present::begin_frame(&mut host);
    present::load_swapchain(&mut host, Some(0x21));
    present::begin_scene(&mut host);
    present::clear(&mut host, CLEAR_COLOR, COLOR, 1.0, 0);
    present::draw(&mut host, TRISTRIP, 0, 4);
    present::end_scene(&mut host);
    present::end_scene(&mut host);
    let _ = present::is_present_ready(&mut host);
    present::present(&mut host);
    present::present(&mut host);
    present::flush(&mut host);
    present::begin_scene(&mut host);

    let expected = [
        Call::BeginFrame,
        Call::LoadSwapchain(Some(0x21)),
        Call::BeginScene,
        Call::Clear(CLEAR_COLOR, COLOR, 1.0f32.to_bits(), 0),
        Call::Draw(TRISTRIP, 0, 4),
        Call::EndScene,
        Call::EndScene,
        Call::IsPresentReady,
        Call::Present,
        Call::Present,
        Call::Flush,
        Call::BeginScene,
    ];
    let calls: Vec<_> = host
        .events
        .iter()
        .map(|&(device, call)| {
            assert_eq!(device, 0);
            call
        })
        .collect();
    assert_eq!(calls, expected);
    assert!(host.logs.is_empty());
}

#[test]
fn test_present_entrypoints_names() {
    let entries = [
        (Entry::BeginFrame, "gs_begin_frame"),
        (Entry::BeginScene, "gs_begin_scene"),
        (Entry::Draw, "gs_draw"),
        (Entry::EndScene, "gs_end_scene"),
        (Entry::LoadSwapchain, "gs_load_swapchain"),
        (Entry::Clear, "gs_clear"),
        (Entry::IsPresentReady, "gs_is_present_ready"),
        (Entry::Present, "gs_present"),
        (Entry::Flush, "gs_flush"),
    ];
    for (entry, name) in entries {
        assert_eq!(entry.name(), name);
        assert_eq!(entry.c_name().to_str(), Ok(name));
    }
}
