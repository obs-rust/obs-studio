//! Tier 1 mirror of `test/cmocka/test_texture_render.c`, one test per C
//! case, through the safe core.
//!
//! [`Fake`] plays both halves of the C fixture: what `graphics.c` does with
//! each call (viewport stack, matrix stack, a null destroy returning before
//! the device) and the recording fake device under it. Its log uses the C
//! test's format, so each expected string is the C test's string.
//!
//! `null_test` and the `bnum_allocs` checks of `create_destroy_test` are
//! about the C handle, which the safe API does not have. Tier 3 covers
//! them on the shim (`texture_render_parity.rs`).

use obs_graphics::texture_render::{
    ColorFormat, ColorSpace, Device, RENDER_TARGET, TexRender, ZStencilFormat,
};

const GS_A8: ColorFormat = ColorFormat(1);
const GS_RGBA: ColorFormat = ColorFormat(3);
const GS_BGRA: ColorFormat = ColorFormat(5);
const GS_R10G10B10A2: ColorFormat = ColorFormat(6);
const GS_RGBA16F: ColorFormat = ColorFormat(9);
const GS_ZS_NONE: ZStencilFormat = ZStencilFormat::NONE;
const GS_Z16: ZStencilFormat = ZStencilFormat(1);
const GS_Z24_S8: ZStencilFormat = ZStencilFormat(2);
const GS_Z32F: ZStencilFormat = ZStencilFormat(3);
const GS_CS_SRGB_16F: ColorSpace = ColorSpace(1);
const GS_CS_709_EXTENDED: ColorSpace = ColorSpace(2);
const GS_CS_709_SCRGB: ColorSpace = ColorSpace(3);

#[derive(Clone, Copy, Debug, PartialEq)]
enum Tex {
    Screen,
    T(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Zs {
    Screen,
    Z(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Matrix {
    Identity,
    Scaled,
}

type Rect = (i32, i32, i32, i32);

struct Fake {
    log: Vec<String>,
    next_texture: usize,
    next_zstencil: usize,
    fail_texture: bool,
    fail_zstencil: bool,
    target: Option<Tex>,
    zs: Option<Zs>,
    space: ColorSpace,
    viewport: Rect,
    viewport_stack: Vec<Rect>,
    matrices: Vec<Matrix>,
    projection_depth: i32,
}

/// The C fixture's `setup`.
fn setup() -> Fake {
    Fake {
        log: Vec::new(),
        next_texture: 0,
        next_zstencil: 0,
        fail_texture: false,
        fail_zstencil: false,
        target: Some(Tex::Screen),
        zs: Some(Zs::Screen),
        space: GS_CS_709_EXTENDED,
        viewport: (1, 2, 300, 200),
        viewport_stack: Vec::new(),
        matrices: vec![Matrix::Identity],
        projection_depth: 0,
    }
}

fn tex_name(texture: Option<Tex>) -> String {
    match texture {
        None => "-".into(),
        Some(Tex::Screen) => "S".into(),
        Some(Tex::T(n)) => format!("T{n}"),
    }
}

fn zs_name(zstencil: Option<Zs>) -> String {
    match zstencil {
        None => "-".into(),
        Some(Zs::Screen) => "SZ".into(),
        Some(Zs::Z(n)) => format!("Z{n}"),
    }
}

impl Fake {
    fn log(&self) -> String {
        self.log.join(";")
    }
    fn clear_log(&mut self) {
        self.log.clear();
    }
    fn cur_matrix(&self) -> usize {
        self.matrices.len() - 1
    }
    fn top(&self) -> Matrix {
        *self.matrices.last().unwrap()
    }
    fn set_scaled_matrix(&mut self) {
        *self.matrices.last_mut().unwrap() = Matrix::Scaled;
    }
}

impl Device for Fake {
    type Texture = Tex;
    type ZStencil = Zs;

    fn texture_create(
        &mut self,
        cx: u32,
        cy: u32,
        format: ColorFormat,
        levels: u32,
        flags: u32,
    ) -> Option<Tex> {
        self.log.push(format!(
            "tex_create {cx} {cy} f{} l{levels} null 0x{flags:x}",
            format.0
        ));
        if self.fail_texture {
            return None;
        }
        self.next_texture += 1;
        Some(Tex::T(self.next_texture))
    }
    fn zstencil_create(&mut self, cx: u32, cy: u32, format: ZStencilFormat) -> Option<Zs> {
        self.log.push(format!("zs_create {cx} {cy} f{}", format.0));
        if self.fail_zstencil {
            return None;
        }
        self.next_zstencil += 1;
        Some(Zs::Z(self.next_zstencil))
    }
    fn texture_destroy(&mut self, texture: Option<Tex>) {
        // graphics.c returns before the device for a null texture.
        if texture.is_some() {
            self.log.push(format!("tex_destroy {}", tex_name(texture)));
        }
    }
    fn zstencil_destroy(&mut self, zstencil: Option<Zs>) {
        if zstencil.is_some() {
            self.log.push(format!("zs_destroy {}", zs_name(zstencil)));
        }
    }
    fn viewport_push(&mut self) {
        self.log.push("vp_get".into());
        self.viewport_stack.push(self.viewport);
    }
    fn viewport_pop(&mut self) {
        if let Some((x, y, cx, cy)) = self.viewport_stack.last().copied() {
            self.set_viewport(x, y, cx, cy);
            self.viewport_stack.pop();
        }
    }
    fn projection_push(&mut self) {
        self.log.push("proj_push".into());
        self.projection_depth += 1;
    }
    fn projection_pop(&mut self) {
        self.log.push("proj_pop".into());
        self.projection_depth -= 1;
    }
    fn matrix_push(&mut self) {
        self.matrices.push(self.top());
    }
    fn matrix_pop(&mut self) {
        // graphics.c logs and keeps the last matrix.
        if self.matrices.len() > 1 {
            self.matrices.pop();
        }
    }
    fn matrix_identity(&mut self) {
        *self.matrices.last_mut().unwrap() = Matrix::Identity;
    }
    fn render_target(&mut self) -> Option<Tex> {
        self.log.push(format!("rt_get {}", tex_name(self.target)));
        self.target
    }
    fn zstencil_target(&mut self) -> Option<Zs> {
        self.log.push(format!("zs_get {}", zs_name(self.zs)));
        self.zs
    }
    fn color_space(&mut self) -> ColorSpace {
        self.log.push(format!("cs_get {}", self.space.0));
        self.space
    }
    fn set_render_target(&mut self, texture: Option<Tex>, zstencil: Option<Zs>, space: ColorSpace) {
        self.log.push(format!(
            "rt_set {} {} cs{}",
            tex_name(texture),
            zs_name(zstencil),
            space.0
        ));
        self.target = texture;
        self.zs = zstencil;
        self.space = space;
    }
    fn set_viewport(&mut self, x: i32, y: i32, cx: i32, cy: i32) {
        self.log.push(format!("vp_set {x} {y} {cx} {cy}"));
        self.viewport = (x, y, cx, cy);
    }
}

#[test]
fn create_destroy_test() {
    let mut g = setup();
    let tr = TexRender::<Fake>::new(GS_RGBA, GS_ZS_NONE);
    assert_eq!(tr.texture(), None);
    assert_eq!(tr.format(), GS_RGBA);
    tr.destroy(&mut g);
    assert_eq!(g.log(), "");

    let mut tr = TexRender::new(GS_R10G10B10A2, GS_Z24_S8);
    assert_eq!(tr.format(), GS_R10G10B10A2);
    assert!(tr.begin(&mut g, 8, 4));
    tr.end(&mut g);
    g.clear_log();
    tr.destroy(&mut g);
    assert_eq!(g.log(), "tex_destroy T1;zs_destroy Z1");
}

#[test]
fn begin_end_test() {
    let mut g = setup();
    let mut tr = TexRender::new(GS_RGBA, GS_ZS_NONE);

    g.set_scaled_matrix();
    assert!(tr.begin(&mut g, 64, 32));
    assert_eq!(
        g.log(),
        "tex_create 64 32 f3 l1 null 0x4;vp_get;proj_push;rt_get S;zs_get SZ;cs_get 2;\
         rt_set T1 - cs0;vp_set 0 0 64 32"
    );
    assert_eq!(tr.texture(), Some(Tex::T(1)));
    assert_eq!(g.top(), Matrix::Identity);
    assert_eq!(g.cur_matrix(), 1);
    assert_eq!(g.viewport_stack.len(), 1);
    assert_eq!(g.projection_depth, 1);

    g.clear_log();
    tr.end(&mut g);
    assert_eq!(g.log(), "rt_set S SZ cs2;proj_pop;vp_set 1 2 300 200");
    assert_eq!(g.top(), Matrix::Scaled);
    assert_eq!(g.cur_matrix(), 0);
    assert_eq!(g.viewport_stack.len(), 0);
    assert_eq!(g.projection_depth, 0);
    assert_eq!(tr.texture(), Some(Tex::T(1)));

    tr.destroy(&mut g);
}

#[test]
fn zstencil_test() {
    let mut g = setup();
    let mut tr = TexRender::new(GS_BGRA, GS_Z24_S8);

    assert!(tr.begin(&mut g, 16, 8));
    assert_eq!(
        g.log(),
        "tex_create 16 8 f5 l1 null 0x4;zs_create 16 8 f2;vp_get;proj_push;\
         rt_get S;zs_get SZ;cs_get 2;rt_set T1 Z1 cs0;vp_set 0 0 16 8"
    );
    g.clear_log();
    tr.end(&mut g);
    assert_eq!(g.log(), "rt_set S SZ cs2;proj_pop;vp_set 1 2 300 200");
    tr.destroy(&mut g);
}

#[test]
fn rendered_reset_test() {
    let mut g = setup();
    let mut tr = TexRender::new(GS_RGBA, GS_ZS_NONE);

    assert!(tr.begin(&mut g, 64, 32));
    tr.end(&mut g);

    g.clear_log();
    assert!(!tr.begin(&mut g, 64, 32));
    assert!(!tr.begin(&mut g, 128, 64));
    assert_eq!(g.log(), "");

    tr.reset();
    tr.reset();
    assert!(tr.begin(&mut g, 64, 32));
    assert_eq!(
        g.log(),
        "vp_get;proj_push;rt_get S;zs_get SZ;cs_get 2;rt_set T1 - cs0;vp_set 0 0 64 32"
    );
    tr.end(&mut g);
    tr.destroy(&mut g);
}

#[test]
fn resize_test() {
    let mut g = setup();
    let mut tr = TexRender::new(GS_RGBA16F, GS_Z32F);

    assert!(tr.begin(&mut g, 64, 32));
    tr.end(&mut g);
    tr.reset();

    g.clear_log();
    assert!(tr.begin(&mut g, 64, 33));
    assert_eq!(
        g.log(),
        "tex_destroy T1;zs_destroy Z1;tex_create 64 33 f9 l1 null 0x4;zs_create 64 33 f3;\
         vp_get;proj_push;rt_get S;zs_get SZ;cs_get 2;rt_set T2 Z2 cs0;vp_set 0 0 64 33"
    );
    assert_eq!(tr.texture(), Some(Tex::T(2)));
    tr.end(&mut g);
    tr.reset();

    g.clear_log();
    assert!(tr.begin(&mut g, 65, 33));
    assert_eq!(
        g.log(),
        "tex_destroy T2;zs_destroy Z2;tex_create 65 33 f9 l1 null 0x4;zs_create 65 33 f3;\
         vp_get;proj_push;rt_get S;zs_get SZ;cs_get 2;rt_set T3 Z3 cs0;vp_set 0 0 65 33"
    );
    tr.end(&mut g);
    tr.destroy(&mut g);
}

#[test]
fn zero_size_test() {
    let mut g = setup();
    let mut tr = TexRender::new(GS_RGBA, GS_ZS_NONE);

    assert!(!tr.begin(&mut g, 0, 32));
    assert!(!tr.begin(&mut g, 32, 0));
    assert!(!tr.begin(&mut g, 0, 0));
    assert_eq!(g.log(), "");
    assert_eq!(tr.texture(), None);

    assert!(tr.begin(&mut g, 32, 32));
    tr.end(&mut g);
    tr.reset();
    g.clear_log();
    assert!(!tr.begin(&mut g, 0, 32));
    assert_eq!(g.log(), "");
    assert_eq!(tr.texture(), Some(Tex::T(1)));
    tr.destroy(&mut g);
}

#[test]
fn texture_failure_test() {
    let mut g = setup();
    let mut tr = TexRender::new(GS_RGBA, GS_Z16);

    g.fail_texture = true;
    assert!(!tr.begin(&mut g, 64, 32));
    assert_eq!(g.log(), "tex_create 64 32 f3 l1 null 0x4");
    assert_eq!(tr.texture(), None);

    // Characterized, not endorsed: the size is stored, so the same size
    // does not retry.
    g.fail_texture = false;
    g.clear_log();
    assert!(!tr.begin(&mut g, 64, 32));
    assert_eq!(g.log(), "");

    assert!(tr.begin(&mut g, 64, 31));
    assert_eq!(
        g.log(),
        "tex_create 64 31 f3 l1 null 0x4;zs_create 64 31 f1;vp_get;proj_push;\
         rt_get S;zs_get SZ;cs_get 2;rt_set T1 Z1 cs0;vp_set 0 0 64 31"
    );
    tr.end(&mut g);
    assert_eq!(g.cur_matrix(), 0);
    assert_eq!(g.viewport_stack.len(), 0);
    tr.destroy(&mut g);
}

#[test]
fn zstencil_failure_test() {
    let mut g = setup();
    let mut tr = TexRender::new(GS_RGBA, GS_Z24_S8);

    g.fail_zstencil = true;
    assert!(!tr.begin(&mut g, 64, 32));
    assert_eq!(
        g.log(),
        "tex_create 64 32 f3 l1 null 0x4;zs_create 64 32 f2;tex_destroy T1"
    );
    assert_eq!(tr.texture(), None);
    assert_eq!(g.cur_matrix(), 0);
    assert_eq!(g.viewport_stack.len(), 0);
    assert_eq!(g.projection_depth, 0);

    g.fail_zstencil = false;
    g.clear_log();
    assert!(!tr.begin(&mut g, 64, 32));
    assert_eq!(g.log(), "");

    g.clear_log();
    tr.destroy(&mut g);
    assert_eq!(g.log(), "");
}

#[test]
fn color_space_test() {
    let mut g = setup();
    let mut tr = TexRender::new(GS_RGBA16F, GS_ZS_NONE);

    g.space = GS_CS_SRGB_16F;
    assert!(tr.begin_with_color_space(&mut g, 10, 20, GS_CS_709_SCRGB));
    assert_eq!(
        g.log(),
        "tex_create 10 20 f9 l1 null 0x4;vp_get;proj_push;rt_get S;zs_get SZ;cs_get 1;\
         rt_set T1 - cs3;vp_set 0 0 10 20"
    );
    assert_eq!(g.space, GS_CS_709_SCRGB);
    g.clear_log();
    tr.end(&mut g);
    assert_eq!(g.log(), "rt_set S SZ cs1;proj_pop;vp_set 1 2 300 200");
    assert_eq!(g.space, GS_CS_SRGB_16F);
    assert_eq!(tr.format(), GS_RGBA16F);

    tr.reset();
    g.clear_log();
    assert!(tr.begin(&mut g, 10, 20));
    assert_eq!(
        g.log(),
        "vp_get;proj_push;rt_get S;zs_get SZ;cs_get 1;rt_set T1 - cs0;vp_set 0 0 10 20"
    );
    tr.end(&mut g);
    tr.destroy(&mut g);
}

#[test]
fn double_begin_test() {
    let mut g = setup();
    let mut tr = TexRender::new(GS_RGBA, GS_ZS_NONE);

    g.set_scaled_matrix();
    assert!(tr.begin(&mut g, 64, 32));
    g.clear_log();
    assert!(tr.begin(&mut g, 64, 32));
    assert_eq!(
        g.log(),
        "vp_get;proj_push;rt_get T1;zs_get -;cs_get 0;rt_set T1 - cs0;vp_set 0 0 64 32"
    );
    assert_eq!(g.cur_matrix(), 2);
    assert_eq!(g.viewport_stack.len(), 2);
    assert_eq!(g.projection_depth, 2);

    // Characterized, not endorsed: the second begin saved the texrender's
    // own target, so neither end restores S.
    g.clear_log();
    tr.end(&mut g);
    assert_eq!(g.log(), "rt_set T1 - cs0;proj_pop;vp_set 0 0 64 32");
    g.clear_log();
    tr.end(&mut g);
    assert_eq!(g.log(), "rt_set T1 - cs0;proj_pop;vp_set 1 2 300 200");
    assert_eq!(g.target, Some(Tex::T(1)));
    assert_eq!(g.top(), Matrix::Scaled);
    assert_eq!(g.cur_matrix(), 0);
    assert_eq!(g.viewport_stack.len(), 0);
    assert_eq!(g.projection_depth, 0);

    tr.destroy(&mut g);
}

#[test]
fn end_without_begin_test() {
    let mut g = setup();
    let mut tr = TexRender::new(GS_RGBA, GS_ZS_NONE);

    // Characterized, not endorsed: the zeroed previous state is restored,
    // the empty stacks do not pop, and rendered is set.
    tr.end(&mut g);
    assert_eq!(g.log(), "rt_set - - cs0;proj_pop");
    assert_eq!(g.cur_matrix(), 0);
    assert_eq!(g.viewport_stack.len(), 0);
    assert_eq!(g.projection_depth, -1);

    g.clear_log();
    assert!(!tr.begin(&mut g, 64, 32));
    assert_eq!(g.log(), "");
    tr.reset();
    assert!(tr.begin(&mut g, 64, 32));
    tr.end(&mut g);

    // A second end after a pair restores the same previous state again.
    g.clear_log();
    tr.end(&mut g);
    assert_eq!(g.log(), "rt_set - - cs0;proj_pop");
    tr.destroy(&mut g);
}

#[test]
fn viewport_cast_test() {
    let mut g = setup();
    let mut tr = TexRender::new(GS_A8, GS_ZS_NONE);

    assert!(tr.begin(&mut g, 0x8000_0000, 0xffff_ffff));
    assert_eq!(
        g.log(),
        "tex_create 2147483648 4294967295 f1 l1 null 0x4;vp_get;proj_push;\
         rt_get S;zs_get SZ;cs_get 2;rt_set T1 - cs0;vp_set 0 0 -2147483648 -1"
    );
    tr.end(&mut g);
    tr.destroy(&mut g);
}

/// Edge case beyond the C test: every texture is a single-level render
/// target, whatever the format.
#[test]
fn texture_flags_edge() {
    let mut g = setup();
    let mut tr = TexRender::new(ColorFormat(0x7fff_ffff), ZStencilFormat(0x7fff_ffff));
    assert!(tr.begin(&mut g, 1, 1));
    assert_eq!(RENDER_TARGET, 4);
    assert!(
        g.log()
            .starts_with("tex_create 1 1 f2147483647 l1 null 0x4;zs_create 1 1 f2147483647;")
    );
    tr.end(&mut g);
    tr.destroy(&mut g);
}
