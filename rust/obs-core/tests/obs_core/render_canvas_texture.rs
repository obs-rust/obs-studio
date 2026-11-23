//! Tier 1: every case of test_render_canvas_texture.c over a synchronous
//! model of the canvases, their mixes and the graphics device: a blend
//! stack, the framebuffer sRGB flag and one target pixel per half that each
//! sprite draw blends into with the current factors. A mix's texture is the
//! test's box, (r, g, b, 0) in its left half and (r, g, b, 1) in its right.
//! Plus what the C test cannot reach: a mix whose texture is not rendered,
//! a NULL main canvas or canvas (the one intentional difference: the C
//! crashes), the colour-space paths (technique, white level, multiplier),
//! the exact call order and the effect's pass count.
use obs_core::canvas_draw::{
    GS_CS_709_EXTENDED, GS_CS_709_SCRGB, GS_CS_SRGB, GS_CS_SRGB_16F, Technique,
};
use obs_core::render_canvas_texture::{
    Blend, GS_BLEND_INVSRCALPHA, GS_BLEND_ONE, GS_BLEND_ZERO, Graphics, OVER, Param,
    SRC_COLOR_ONLY, render_canvas, render_main,
};

const MAIN: usize = 0;
const AUX: usize = 1;
const BARE: usize = 2;

const RED: [u8; 3] = [255, 0, 0];
const BLUE: [u8; 3] = [0, 0, 255];
/// The target as cleared: green at half alpha.
const UNCHANGED: [u8; 4] = [0, 255, 0, 128];

#[derive(Clone, Copy, Debug)]
struct Mix {
    rendered: bool,
    space: i32,
    /// The texture id: the box colour index (0 red, 1 blue), or `None` for
    /// a NULL texture.
    texture: Option<usize>,
}

/// A graphics call or field read, in order.
#[derive(Clone, Debug, PartialEq)]
enum Call {
    MainCanvas,
    Mix(usize),
    Rendered(usize),
    Space(usize),
    Texture(usize),
    ColorSpace,
    WhiteLevel,
    SrgbEnabled,
    EnableSrgb(bool),
    DefaultEffect,
    Param(&'static str),
    SetTextureSrgb(&'static str, Option<usize>),
    SetFloat(&'static str, u32),
    BlendPush,
    BlendFunction(Blend),
    Loop(Technique),
    Draw(Option<usize>),
    BlendPop,
}

struct World {
    /// `obs->data.main_canvas`, an index into `canvases`.
    main: Option<usize>,
    /// Each canvas's mix, an index into `mixes`.
    canvases: Vec<Option<usize>>,
    mixes: Vec<Mix>,
    colors: [[u8; 3]; 2],
    color_space: i32,
    white_level: f32,
    passes: usize,
    loops: usize,
    srgb: bool,
    blend: Blend,
    stack: Vec<Blend>,
    /// The left and right target pixels, 0..1 floats.
    target: [[f32; 4]; 2],
    calls: Vec<Call>,
}

const CALLER: Blend = Blend {
    src_color: GS_BLEND_ZERO,
    dest_color: GS_BLEND_ONE,
    src_alpha: GS_BLEND_ZERO,
    dest_alpha: GS_BLEND_ONE,
};

impl World {
    /// The test's setup: the main canvas shows the red box, an auxiliary
    /// canvas with its own mix the blue box, and a bare canvas has no mix.
    fn new() -> Self {
        World {
            main: Some(MAIN),
            canvases: vec![Some(0), Some(1), None],
            mixes: vec![
                Mix {
                    rendered: true,
                    space: GS_CS_SRGB,
                    texture: Some(0),
                },
                Mix {
                    rendered: true,
                    space: GS_CS_SRGB,
                    texture: Some(1),
                },
            ],
            colors: [RED, BLUE],
            color_space: GS_CS_SRGB,
            white_level: 300.0,
            passes: 1,
            loops: 0,
            srgb: false,
            blend: CALLER,
            stack: Vec::new(),
            target: [[0.0; 4]; 2],
            calls: Vec::new(),
        }
    }

    fn texture_pixel(&self, texture: usize, right: bool) -> [f32; 4] {
        let [r, g, b] = self.colors[texture].map(|c| f32::from(c) / 255.0);
        [r, g, b, if right { 1.0 } else { 0.0 }]
    }

    /// Blends `src` into the target pixel with the current factors (only the
    /// three factors these draws use; anything else is a test failure).
    fn blend_into(&mut self, half: usize, src: [f32; 4]) {
        let factor = |f: u32, src_alpha: f32| match f {
            GS_BLEND_ZERO => 0.0,
            GS_BLEND_ONE => 1.0,
            GS_BLEND_INVSRCALPHA => 1.0 - src_alpha,
            other => panic!("blend factor {other} not modelled"),
        };
        let dst = self.target[half];
        let a = src[3];
        let mut out = [0.0; 4];
        for i in 0..3 {
            out[i] = src[i] * factor(self.blend.src_color, a)
                + dst[i] * factor(self.blend.dest_color, a);
        }
        out[3] =
            src[3] * factor(self.blend.src_alpha, a) + dst[3] * factor(self.blend.dest_alpha, a);
        self.target[half] = out.map(|v| v.clamp(0.0, 1.0));
    }
}

impl Graphics for World {
    type Canvas = usize;
    type Mix = usize;
    type Texture = Option<usize>;
    type Effect = ();
    type Param = &'static str;

    fn main_canvas(&mut self) -> Option<usize> {
        self.calls.push(Call::MainCanvas);
        self.main
    }
    fn mix(&mut self, canvas: usize) -> Option<usize> {
        self.calls.push(Call::Mix(canvas));
        self.canvases[canvas]
    }
    fn texture_rendered(&mut self, mix: usize) -> bool {
        self.calls.push(Call::Rendered(mix));
        self.mixes[mix].rendered
    }
    fn render_space(&mut self, mix: usize) -> i32 {
        self.calls.push(Call::Space(mix));
        self.mixes[mix].space
    }
    fn render_texture(&mut self, mix: usize) -> Option<usize> {
        self.calls.push(Call::Texture(mix));
        self.mixes[mix].texture
    }
    fn color_space(&mut self) -> i32 {
        self.calls.push(Call::ColorSpace);
        self.color_space
    }
    fn sdr_white_level(&mut self) -> f32 {
        self.calls.push(Call::WhiteLevel);
        self.white_level
    }
    fn framebuffer_srgb_enabled(&mut self) -> bool {
        self.calls.push(Call::SrgbEnabled);
        self.srgb
    }
    fn enable_framebuffer_srgb(&mut self, enable: bool) {
        self.calls.push(Call::EnableSrgb(enable));
        self.srgb = enable;
    }
    fn default_effect(&mut self) {
        self.calls.push(Call::DefaultEffect);
    }
    fn param(&mut self, _effect: (), param: Param) -> &'static str {
        let name = param.name().to_str().unwrap();
        self.calls.push(Call::Param(name));
        name
    }
    fn set_texture_srgb(&mut self, param: &'static str, texture: Option<usize>) {
        self.calls.push(Call::SetTextureSrgb(param, texture));
    }
    fn set_float(&mut self, param: &'static str, value: f32) {
        self.calls.push(Call::SetFloat(param, value.to_bits()));
    }
    fn blend_state_push(&mut self) {
        self.calls.push(Call::BlendPush);
        self.stack.push(self.blend);
    }
    fn blend_function_separate(&mut self, blend: Blend) {
        self.calls.push(Call::BlendFunction(blend));
        self.blend = blend;
    }
    fn effect_loop(&mut self, _effect: (), technique: Technique) -> bool {
        self.calls.push(Call::Loop(technique));
        if self.loops < self.passes {
            self.loops += 1;
            true
        } else {
            self.loops = 0;
            false
        }
    }
    fn draw_sprite(&mut self, texture: Option<usize>) {
        self.calls.push(Call::Draw(texture));
        if let Some(texture) = texture {
            for half in 0..2 {
                let src = self.texture_pixel(texture, half == 1);
                self.blend_into(half, src);
            }
        }
    }
    fn blend_state_pop(&mut self) {
        self.calls.push(Call::BlendPop);
        self.blend = self.stack.pop().expect("blend pop without push");
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Entry {
    MainTexture,
    MainTextureSrcColorOnly,
    CanvasTexture,
    CanvasTextureSrcColorOnly,
}

fn call_entry(world: &mut World, entry: Entry, canvas: Option<usize>) {
    match entry {
        Entry::MainTexture => render_main(world, OVER),
        Entry::MainTextureSrcColorOnly => render_main(world, SRC_COLOR_ONLY),
        Entry::CanvasTexture => render_canvas(world, canvas, OVER),
        Entry::CanvasTextureSrcColorOnly => render_canvas(world, canvas, SRC_COLOR_ONLY),
    }
}

struct Readback {
    left: [u8; 4],
    right: [u8; 4],
    srgb_after: bool,
}

fn to_u8(pixel: [f32; 4]) -> [u8; 4] {
    pixel.map(|v| (v * 255.0).round() as u8)
}

/// The C test's draw(): clear to (0, 1, 0, 0.5), the caller's blend ZERO,
/// ONE and sRGB `srgb_before`, the entry point, then an opaque white quad
/// the caller's blend must keep out.
fn draw(world: &mut World, entry: Entry, canvas: Option<usize>, srgb_before: bool) -> Readback {
    world.target = [[0.0, 1.0, 0.0, 0.5]; 2];
    world.srgb = srgb_before;
    world.blend = CALLER;
    world.stack.clear();
    world.calls.clear();
    call_entry(world, entry, canvas);
    let srgb_after = world.srgb;
    assert!(world.stack.is_empty(), "blend stack not balanced");
    for half in 0..2 {
        world.blend_into(half, [1.0; 4]);
    }
    Readback {
        left: to_u8(world.target[0]),
        right: to_u8(world.target[1]),
        srgb_after,
    }
}

fn expected(rgb: [u8; 3], src_color_only: bool) -> ([u8; 4], [u8; 4]) {
    (
        [
            rgb[0],
            if src_color_only { rgb[1] } else { 255 },
            rgb[2],
            128,
        ],
        [rgb[0], rgb[1], rgb[2], 255],
    )
}

fn near(got: [u8; 4], want: [u8; 4]) -> bool {
    got.iter()
        .zip(want)
        .all(|(&g, w)| (i32::from(g) - i32::from(w)).abs() <= 2)
}

fn check(world: &mut World, entry: Entry, canvas: Option<usize>, left: [u8; 4], right: [u8; 4]) {
    for srgb in [false, true] {
        let rb = draw(world, entry, canvas, srgb);
        assert!(
            near(rb.left, left) && near(rb.right, right),
            "sRGB {srgb}: left {:?} right {:?}, want {left:?} {right:?}",
            rb.left,
            rb.right
        );
        assert_eq!(rb.srgb_after, srgb, "framebuffer sRGB not restored");
    }
}

fn check_box(world: &mut World, entry: Entry, canvas: Option<usize>, rgb: [u8; 3]) {
    let src_color_only = matches!(
        entry,
        Entry::MainTextureSrcColorOnly | Entry::CanvasTextureSrcColorOnly
    );
    let (left, right) = expected(rgb, src_color_only);
    check(world, entry, canvas, left, right);
}

/// test_render_canvas_texture: main_texture_blends_over_destination.
#[test]
fn main_texture_blends_over_destination() {
    let mut world = World::new();
    check_box(&mut world, Entry::MainTexture, None, RED);
}

/// test_render_canvas_texture: main_texture_src_color_only_drops_destination_color.
#[test]
fn main_texture_src_color_only_drops_destination_color() {
    let mut world = World::new();
    check_box(&mut world, Entry::MainTextureSrcColorOnly, None, RED);
}

/// test_render_canvas_texture: canvas_texture_draws_main_canvas.
#[test]
fn canvas_texture_draws_main_canvas() {
    let mut world = World::new();
    check_box(&mut world, Entry::CanvasTexture, Some(MAIN), RED);
    check_box(
        &mut world,
        Entry::CanvasTextureSrcColorOnly,
        Some(MAIN),
        RED,
    );
}

/// test_render_canvas_texture: canvas_texture_draws_given_canvas.
#[test]
fn canvas_texture_draws_given_canvas() {
    let mut world = World::new();
    check_box(&mut world, Entry::CanvasTexture, Some(AUX), BLUE);
    check_box(
        &mut world,
        Entry::CanvasTextureSrcColorOnly,
        Some(AUX),
        BLUE,
    );
}

/// test_render_canvas_texture: canvas_without_mix_draws_nothing. Only the
/// canvas's mix is read.
#[test]
fn canvas_without_mix_draws_nothing() {
    let mut world = World::new();
    check(
        &mut world,
        Entry::CanvasTexture,
        Some(BARE),
        UNCHANGED,
        UNCHANGED,
    );
    check(
        &mut world,
        Entry::CanvasTextureSrcColorOnly,
        Some(BARE),
        UNCHANGED,
        UNCHANGED,
    );
    assert_eq!(world.calls, [Call::Mix(BARE)]);
}

/// test_render_canvas_texture: main_texture_ignores_auxiliary_canvas. The
/// auxiliary canvas renders new frames (its mix changes) between the draws;
/// the main-texture variants still draw the main canvas.
#[test]
fn main_texture_ignores_auxiliary_canvas() {
    let mut world = World::new();
    check_box(&mut world, Entry::MainTexture, None, RED);
    check_box(&mut world, Entry::MainTextureSrcColorOnly, None, RED);
    world.mixes[AUX].space = GS_CS_709_EXTENDED;
    world.colors[1] = [0, 0, 255];
    check_box(&mut world, Entry::MainTexture, None, RED);
    check_box(&mut world, Entry::MainTextureSrcColorOnly, None, RED);
    assert!(!world.calls.contains(&Call::Mix(AUX)));
}

/// Not reachable from the C test: a mix whose texture is not rendered draws
/// nothing; the flag is read and nothing after it.
#[test]
fn unrendered_mix_draws_nothing() {
    let mut world = World::new();
    world.mixes[0].rendered = false;
    check(&mut world, Entry::MainTexture, None, UNCHANGED, UNCHANGED);
    assert_eq!(
        world.calls,
        [Call::MainCanvas, Call::Mix(MAIN), Call::Rendered(0)]
    );
    check(
        &mut world,
        Entry::CanvasTextureSrcColorOnly,
        Some(MAIN),
        UNCHANGED,
        UNCHANGED,
    );
    assert_eq!(world.calls, [Call::Mix(MAIN), Call::Rendered(0)]);
}

/// Not reachable from the C test, and the one intentional difference: a
/// NULL main canvas or a NULL canvas draws nothing (the C crashes).
#[test]
fn null_canvas_draws_nothing() {
    let mut world = World::new();
    world.main = None;
    check(&mut world, Entry::MainTexture, None, UNCHANGED, UNCHANGED);
    assert_eq!(world.calls, [Call::MainCanvas]);
    check(
        &mut world,
        Entry::MainTextureSrcColorOnly,
        None,
        UNCHANGED,
        UNCHANGED,
    );
    for entry in [Entry::CanvasTexture, Entry::CanvasTextureSrcColorOnly] {
        check(&mut world, entry, None, UNCHANGED, UNCHANGED);
        assert!(world.calls.is_empty());
    }
}

/// The whole call sequence of a draw, in the C's order, for each variant.
#[test]
fn calls_in_the_c_order() {
    for (entry, blend) in [
        (Entry::MainTexture, OVER),
        (Entry::MainTextureSrcColorOnly, SRC_COLOR_ONLY),
        (Entry::CanvasTexture, OVER),
        (Entry::CanvasTextureSrcColorOnly, SRC_COLOR_ONLY),
    ] {
        let mut world = World::new();
        let main = matches!(entry, Entry::MainTexture | Entry::MainTextureSrcColorOnly);
        draw(&mut world, entry, Some(AUX), true);
        let (canvas, mix) = if main { (MAIN, 0) } else { (AUX, 1) };
        let mut want = Vec::new();
        if main {
            want.push(Call::MainCanvas);
        }
        want.extend([
            Call::Mix(canvas),
            Call::Rendered(mix),
            Call::Space(mix),
            Call::ColorSpace,
            Call::SrgbEnabled,
            Call::EnableSrgb(true),
            Call::Texture(mix),
            Call::DefaultEffect,
            Call::Param("image"),
            Call::SetTextureSrgb("image", Some(mix)),
            Call::Param("multiplier"),
            Call::SetFloat("multiplier", 1.0f32.to_bits()),
            Call::BlendPush,
            Call::BlendFunction(blend),
            Call::Loop(Technique::Draw),
            Call::Draw(Some(mix)),
            Call::Loop(Technique::Draw),
            Call::BlendPop,
            Call::EnableSrgb(true),
        ]);
        assert_eq!(world.calls, want);
    }
}

/// The blend factors of the two variants, as the C passes them.
#[test]
fn blend_factors() {
    assert_eq!(
        OVER,
        Blend {
            src_color: GS_BLEND_ONE,
            dest_color: GS_BLEND_INVSRCALPHA,
            src_alpha: GS_BLEND_ONE,
            dest_alpha: GS_BLEND_INVSRCALPHA
        }
    );
    assert_eq!(
        SRC_COLOR_ONLY,
        Blend {
            src_color: GS_BLEND_ONE,
            dest_color: GS_BLEND_ZERO,
            src_alpha: GS_BLEND_ONE,
            dest_alpha: GS_BLEND_INVSRCALPHA
        }
    );
}

/// The technique, the white level read and the multiplier by colour space:
/// into scRGB the white level is read and the multiplier is it over 80;
/// a 709 extended canvas into SDR tonemaps; otherwise Draw at 1.
#[test]
fn colour_spaces_pick_technique_and_multiplier() {
    let cases = [
        (GS_CS_SRGB, GS_CS_SRGB, Technique::Draw, false),
        (
            GS_CS_709_EXTENDED,
            GS_CS_SRGB,
            Technique::DrawTonemap,
            false,
        ),
        (
            GS_CS_709_EXTENDED,
            GS_CS_SRGB_16F,
            Technique::DrawTonemap,
            false,
        ),
        (
            GS_CS_709_EXTENDED,
            GS_CS_709_EXTENDED,
            Technique::Draw,
            false,
        ),
        (GS_CS_SRGB, GS_CS_709_SCRGB, Technique::DrawMultiply, true),
        (
            GS_CS_709_EXTENDED,
            GS_CS_709_SCRGB,
            Technique::DrawMultiply,
            true,
        ),
        (GS_CS_SRGB, 99, Technique::Draw, false),
    ];
    for (source, current, technique, reads) in cases {
        let mut world = World::new();
        world.mixes[0].space = source;
        world.color_space = current;
        world.white_level = 300.0;
        render_main(&mut world, OVER);
        assert_eq!(
            world.calls.contains(&Call::WhiteLevel),
            reads,
            "{source} {current}"
        );
        let multiplier = if reads { 300.0f32 / 80.0 } else { 1.0 };
        assert!(
            world
                .calls
                .contains(&Call::SetFloat("multiplier", multiplier.to_bits()))
        );
        assert!(
            world
                .calls
                .iter()
                .all(|c| !matches!(c, Call::Loop(t) if *t != technique))
        );
        if reads {
            let at = world
                .calls
                .iter()
                .position(|c| *c == Call::WhiteLevel)
                .unwrap();
            assert_eq!(world.calls[at - 1], Call::ColorSpace);
            assert_eq!(world.calls[at + 1], Call::SrgbEnabled);
        }
    }
}

/// Each pass of the technique draws the texture once; a NULL texture is
/// passed on as it is; the texture is read once.
#[test]
fn every_pass_draws_and_texture_read_once() {
    for passes in [0, 1, 3] {
        let mut world = World::new();
        world.passes = passes;
        world.mixes[0].texture = None;
        render_main(&mut world, OVER);
        let draws = world
            .calls
            .iter()
            .filter(|c| **c == Call::Draw(None))
            .count();
        let loops = world
            .calls
            .iter()
            .filter(|c| matches!(c, Call::Loop(_)))
            .count();
        assert_eq!((draws, loops), (passes, passes + 1));
        let reads = world
            .calls
            .iter()
            .filter(|c| **c == Call::Texture(0))
            .count();
        assert_eq!(reads, 1);
        assert!(world.calls.contains(&Call::SetTextureSrgb("image", None)));
        assert_eq!(world.calls.last(), Some(&Call::EnableSrgb(false)));
    }
}
