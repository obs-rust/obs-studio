//! Tier 1 mirrors test_graphics_render_target_setters without FFI or unsafe.
use obs_graphics::render_target_setters::{self as core, Entry, Host, NO_CONTEXT};

const DEVICE: u64 = 7;
const TEXTURE: u64 = 11;
const DEPTH: u64 = 13;

#[derive(Debug, PartialEq, Eq)]
struct Call {
    entry: Entry,
    device: u64,
    texture: Option<u64>,
    depth: Option<u64>,
    side: i32,
}

#[derive(Default)]
struct Fixture {
    context: bool,
    calls: Vec<Call>,
    logs: Vec<(i32, String)>,
}

impl Host for Fixture {
    type Texture = u64;
    type Depth = u64;
    fn has_context(&self) -> bool {
        self.context
    }
    fn debug(&mut self, entry: Entry) {
        self.logs.push((
            400,
            format!("{}: {}", entry.name(), NO_CONTEXT.to_str().unwrap()),
        ));
    }
    fn target(&mut self, texture: Option<u64>, depth: Option<u64>) {
        assert!(self.context, "dispatch requires a context");
        self.calls.push(Call {
            entry: Entry::RenderTarget,
            device: DEVICE,
            texture,
            depth,
            side: 0,
        });
    }
    fn cube(&mut self, texture: Option<u64>, side: i32, depth: Option<u64>) {
        assert!(self.context, "dispatch requires a context");
        self.calls.push(Call {
            entry: Entry::CubeRenderTarget,
            device: DEVICE,
            texture,
            depth,
            side,
        });
    }
}

fn diagnostic(f: &Fixture, entry: Entry, expected_logs: usize) {
    assert_eq!(f.logs.len(), expected_logs);
    assert_eq!(
        f.logs.last(),
        Some(&(
            400,
            format!("{}: called while not in a graphics context", entry.name())
        ))
    );
    assert!(f.calls.is_empty());
}

fn forwarded(
    f: &Fixture,
    position: usize,
    entry: Entry,
    texture: Option<u64>,
    depth: Option<u64>,
    side: i32,
) {
    assert_eq!(f.calls.len(), position + 1);
    assert_eq!(
        f.calls[position],
        Call {
            entry,
            device: DEVICE,
            texture,
            depth,
            side
        }
    );
    assert!(f.logs.is_empty());
}

fn handles(mask: u8) -> (Option<u64>, Option<u64>) {
    (
        (mask & 1 != 0).then_some(TEXTURE),
        (mask & 2 != 0).then_some(DEPTH),
    )
}

#[test]
fn test_graphics_render_target_setters_no_context() {
    let mut f = Fixture::default();
    for mask in 0..4 {
        let (texture, depth) = handles(mask);
        core::set_render_target(&mut f, texture, depth);
        diagnostic(&f, Entry::RenderTarget, usize::from(mask) * 2 + 1);
        core::set_cube_render_target(&mut f, texture, i32::MIN, depth);
        diagnostic(&f, Entry::CubeRenderTarget, usize::from(mask) * 2 + 2);
    }
}

#[test]
fn test_graphics_render_target_setters_nullable_render_targets() {
    let mut f = Fixture {
        context: true,
        ..Fixture::default()
    };
    for mask in 0..4 {
        let (texture, depth) = handles(mask);
        core::set_render_target(&mut f, texture, depth);
        forwarded(
            &f,
            usize::from(mask),
            Entry::RenderTarget,
            texture,
            depth,
            0,
        );
    }
    f.context = false;
    f.calls.clear();
    core::set_render_target(&mut f, None, None);
    diagnostic(&f, Entry::RenderTarget, 1);
}

#[test]
fn test_graphics_render_target_setters_nullable_cube_targets_and_faces() {
    let mut f = Fixture {
        context: true,
        ..Fixture::default()
    };
    for mask in 0..4 {
        let (texture, depth) = handles(mask);
        // Characterized, not endorsed: out-of-range faces reach the backend.
        for side in [0, 1, 2, 3, 4, 5, -1, i32::MIN, i32::MAX] {
            let n = f.calls.len();
            core::set_cube_render_target(&mut f, texture, side, depth);
            forwarded(&f, n, Entry::CubeRenderTarget, texture, depth, side);
        }
    }
    f.context = false;
    f.calls.clear();
    core::set_cube_render_target(&mut f, None, i32::MAX, None);
    diagnostic(&f, Entry::CubeRenderTarget, 1);
}

#[test]
fn diagnostic_names_match_c() {
    for entry in [Entry::RenderTarget, Entry::CubeRenderTarget] {
        assert_eq!(entry.name().as_bytes(), entry.c_name().to_bytes());
    }
}
