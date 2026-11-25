//! Tier 1 for `obs_source::source_draw` (#5742): every
//! `test_source_draw` case over a recording model of the device (the
//! current effect, the params, the framebuffer sRGB flag, the matrix stack
//! and the sprites drawn), plus the call order, the lookup order, NULL
//! params from the lookup, the sRGB flag during the draw and the
//! translation of extreme positions.
use obs_source::source_draw::{
    self, DEFAULT_RANGE_MAX, DEFAULT_RANGE_MIN, Function, GS_FLIP_V, Graphics, Param,
};
use std::collections::HashMap;

const LOG_WARNING: i32 = 200;
const LOG_DEBUG: i32 = 400;

const EFFECT: u8 = 7;
const TEXTURE: u8 = 3;

/// A param's value as the model holds it.
#[derive(Debug, Clone, PartialEq)]
enum Value {
    Matrix([f32; 16]),
    Range([f32; 3]),
    Texture { texture: u8, srgb: bool },
}

/// One call, as the model saw it.
#[derive(Debug, Clone, PartialEq)]
enum Call {
    CurrentEffect,
    Log(i32, String),
    Param(Param),
    SetMatrix(Option<Param>),
    SetRange(Option<Param>),
    LinearSrgb,
    SrgbEnabled,
    EnableSrgb(bool),
    SetTextureSrgb(Option<Param>, u8),
    SetTexture(Option<Param>, u8),
    Push,
    Translate(u32, u32, u32),
    Draw {
        texture: u8,
        flip: u32,
        cx: u32,
        cy: u32,
    },
    Pop,
}

/// A sprite as it landed: where, how big, flipped, with the sRGB flag.
#[derive(Debug, Clone, PartialEq)]
struct Sprite {
    texture: u8,
    at: (f32, f32),
    flip: u32,
    cx: u32,
    cy: u32,
    srgb: bool,
    image: Option<Value>,
}

struct Model {
    effect: Option<u8>,
    linear: bool,
    srgb: bool,
    /// Lookups give NULL.
    null_params: bool,
    params: HashMap<Param, Value>,
    stack: Vec<(f32, f32)>,
    at: (f32, f32),
    calls: Vec<Call>,
    sprites: Vec<Sprite>,
}

impl Model {
    fn new(effect: bool, linear: bool, srgb: bool) -> Self {
        Model {
            effect: effect.then_some(EFFECT),
            linear,
            srgb,
            null_params: false,
            params: HashMap::new(),
            stack: Vec::new(),
            at: (0.0, 0.0),
            calls: Vec::new(),
            sprites: Vec::new(),
        }
    }

    fn logs(&self) -> Vec<(i32, String)> {
        self.calls
            .iter()
            .filter_map(|c| match c {
                Call::Log(level, text) => Some((*level, text.clone())),
                _ => None,
            })
            .collect()
    }
}

impl Graphics for Model {
    type Effect = u8;
    type Param = Option<Param>;
    type Texture = u8;

    fn current_effect(&mut self) -> Option<u8> {
        self.calls.push(Call::CurrentEffect);
        self.effect
    }
    fn warn_no_effect(&mut self, function: Function) {
        let text = function.no_effect_warning().to_str().unwrap().to_owned();
        self.calls.push(Call::Log(LOG_WARNING, text));
    }
    fn log_null(&mut self, function: Function) {
        let text = format!(
            "{}: Null '{}' parameter",
            function.name().to_str().unwrap(),
            function.checked_param().to_str().unwrap()
        );
        self.calls.push(Call::Log(LOG_DEBUG, text));
    }
    fn param(&mut self, effect: u8, param: Param) -> Option<Param> {
        assert_eq!(effect, EFFECT);
        self.calls.push(Call::Param(param));
        (!self.null_params).then_some(param)
    }
    fn set_matrix4(&mut self, param: Option<Param>, matrix: &[f32; 16]) {
        self.calls.push(Call::SetMatrix(param));
        if let Some(p) = param {
            self.params.insert(p, Value::Matrix(*matrix));
        }
    }
    fn set_range(&mut self, param: Option<Param>, range: &[f32; 3]) {
        self.calls.push(Call::SetRange(param));
        if let Some(p) = param {
            self.params.insert(p, Value::Range(*range));
        }
    }
    fn linear_srgb(&mut self) -> bool {
        self.calls.push(Call::LinearSrgb);
        self.linear
    }
    fn framebuffer_srgb_enabled(&mut self) -> bool {
        self.calls.push(Call::SrgbEnabled);
        self.srgb
    }
    fn enable_framebuffer_srgb(&mut self, enable: bool) {
        self.calls.push(Call::EnableSrgb(enable));
        self.srgb = enable;
    }
    fn set_texture_srgb(&mut self, param: Option<Param>, texture: u8) {
        self.calls.push(Call::SetTextureSrgb(param, texture));
        if let Some(p) = param {
            self.params.insert(
                p,
                Value::Texture {
                    texture,
                    srgb: true,
                },
            );
        }
    }
    fn set_texture(&mut self, param: Option<Param>, texture: u8) {
        self.calls.push(Call::SetTexture(param, texture));
        if let Some(p) = param {
            self.params.insert(
                p,
                Value::Texture {
                    texture,
                    srgb: false,
                },
            );
        }
    }
    fn matrix_push(&mut self) {
        self.calls.push(Call::Push);
        self.stack.push(self.at);
    }
    fn matrix_translate3f(&mut self, x: f32, y: f32, z: f32) {
        self.calls
            .push(Call::Translate(x.to_bits(), y.to_bits(), z.to_bits()));
        self.at = (self.at.0 + x, self.at.1 + y);
    }
    fn draw_sprite(&mut self, texture: u8, flip: u32, cx: u32, cy: u32) {
        self.calls.push(Call::Draw {
            texture,
            flip,
            cx,
            cy,
        });
        self.sprites.push(Sprite {
            texture,
            at: self.at,
            flip,
            cx,
            cy,
            srgb: self.srgb,
            image: self.params.get(&Param::Image).cloned(),
        });
    }
    fn matrix_pop(&mut self) {
        self.calls.push(Call::Pop);
        self.at = self.stack.pop().expect("a pop without a push");
    }
}

fn matrix(base: f32) -> [f32; 16] {
    core::array::from_fn(|i| base + i as f32)
}

// ---------------------------------------------------------------------------
// obs_source_draw_set_color_matrix

/// Mirrors `color_matrix_without_effect_warns`.
#[test]
fn test_source_draw_color_matrix_without_effect_warns() {
    let m = matrix(1.0);
    for null in [false, true] {
        let mut g = Model::new(false, false, false);
        if null {
            source_draw::set_color_matrix(&mut g, None, None, None);
        } else {
            source_draw::set_color_matrix(&mut g, Some(&m), Some(&[0.25, 0.5, 0.75]), None);
        }
        assert_eq!(
            g.calls,
            [
                Call::CurrentEffect,
                Call::Log(
                    LOG_WARNING,
                    "obs_source_draw_set_color_matrix: no active effect!".into()
                )
            ]
        );
        assert!(g.params.is_empty());
    }
}

/// Mirrors `color_matrix_null_matrix_sets_nothing`.
#[test]
fn test_source_draw_color_matrix_null_matrix_sets_nothing() {
    let min = [0.25, 0.5, 0.75];
    let max = [0.5, 0.75, 1.0];
    for (min, max) in [(Some(&min), Some(&max)), (None, None)] {
        let mut g = Model::new(true, false, false);
        source_draw::set_color_matrix(&mut g, None, min, max);
        assert_eq!(
            g.calls,
            [
                Call::CurrentEffect,
                Call::Log(
                    LOG_DEBUG,
                    "obs_source_draw_set_color_matrix: Null 'color_matrix' parameter".into()
                )
            ]
        );
        assert!(g.params.is_empty());
    }
}

/// Mirrors `color_matrix_sets_matrix_and_ranges`, with the lookup order.
#[test]
fn test_source_draw_color_matrix_sets_matrix_and_ranges() {
    let m = matrix(-3.5);
    let min = [0.0625, 0.125, 0.25];
    let max = [0.5, 0.75, 0.9375];
    let mut g = Model::new(true, false, false);
    source_draw::set_color_matrix(&mut g, Some(&m), Some(&min), Some(&max));
    assert!(g.logs().is_empty());
    assert_eq!(g.params[&Param::ColorMatrix], Value::Matrix(m));
    assert_eq!(g.params[&Param::ColorRangeMin], Value::Range(min));
    assert_eq!(g.params[&Param::ColorRangeMax], Value::Range(max));
    assert_eq!(
        g.calls,
        [
            Call::CurrentEffect,
            Call::Param(Param::ColorMatrix),
            Call::Param(Param::ColorRangeMin),
            Call::Param(Param::ColorRangeMax),
            Call::SetMatrix(Some(Param::ColorMatrix)),
            Call::SetRange(Some(Param::ColorRangeMin)),
            Call::SetRange(Some(Param::ColorRangeMax)),
        ]
    );
}

/// Mirrors `color_matrix_null_ranges_default`.
#[test]
fn test_source_draw_color_matrix_null_ranges_default() {
    let m = matrix(7.0);
    let min = [0.0625, 0.125, 0.25];
    let max = [0.5, 0.75, 0.9375];
    let cases = [
        (None, None, DEFAULT_RANGE_MIN, DEFAULT_RANGE_MAX),
        (None, Some(&max), DEFAULT_RANGE_MIN, max),
        (Some(&min), None, min, DEFAULT_RANGE_MAX),
    ];
    assert_eq!(DEFAULT_RANGE_MIN, [0.0, 0.0, 0.0]);
    assert_eq!(DEFAULT_RANGE_MAX, [1.0, 1.0, 1.0]);
    for (given_min, given_max, want_min, want_max) in cases {
        let mut g = Model::new(true, false, false);
        source_draw::set_color_matrix(&mut g, Some(&m), given_min, given_max);
        assert!(g.logs().is_empty());
        assert_eq!(g.params[&Param::ColorMatrix], Value::Matrix(m));
        assert_eq!(g.params[&Param::ColorRangeMin], Value::Range(want_min));
        assert_eq!(g.params[&Param::ColorRangeMax], Value::Range(want_max));
    }
}

/// NULL params from the lookup are passed on as they came.
#[test]
fn edge_color_matrix_null_params_passed_on() {
    let m = matrix(0.0);
    let mut g = Model::new(true, false, false);
    g.null_params = true;
    source_draw::set_color_matrix(&mut g, Some(&m), None, None);
    assert_eq!(
        &g.calls[4..],
        [
            Call::SetMatrix(None),
            Call::SetRange(None),
            Call::SetRange(None)
        ]
    );
}

// ---------------------------------------------------------------------------
// obs_source_draw

/// Mirrors `draw_null_texture_draws_nothing`: the texture is checked
/// before the effect is read.
#[test]
fn test_source_draw_draw_null_texture_draws_nothing() {
    for effect in [true, false] {
        let mut g = Model::new(effect, true, !effect);
        source_draw::draw(&mut g, None, 1, 1, 0, 0, false);
        assert_eq!(
            g.calls,
            [Call::Log(
                LOG_DEBUG,
                "obs_source_draw: Null 'texture' parameter".into()
            )]
        );
        assert!(g.sprites.is_empty());
        assert!(g.params.is_empty());
        assert_eq!(g.srgb, !effect);
    }
}

/// Mirrors `draw_without_effect_warns`.
#[test]
fn test_source_draw_draw_without_effect_warns() {
    for s in 0..4 {
        let (srgb, linear) = (s & 1 != 0, s & 2 != 0);
        let mut g = Model::new(false, linear, srgb);
        source_draw::draw(&mut g, Some(TEXTURE), 2, 3, 0, 0, true);
        assert_eq!(
            g.calls,
            [
                Call::CurrentEffect,
                Call::Log(LOG_WARNING, "obs_source_draw: no active effect!".into())
            ]
        );
        assert!(g.sprites.is_empty());
        assert_eq!(g.srgb, srgb);
        assert!(g.stack.is_empty());
    }
}

/// Mirrors `draw_sets_image_and_restores_state`; the sRGB flag during the
/// draw is linear sRGB.
#[test]
fn test_source_draw_draw_sets_image_and_restores_state() {
    for s in 0..8 {
        let (srgb, linear, offset) = (s & 1 != 0, s & 2 != 0, s & 4 != 0);
        let (x, y) = if offset { (3, 1) } else { (0, 0) };
        let mut g = Model::new(true, linear, srgb);
        source_draw::draw(&mut g, Some(TEXTURE), x, y, 0, 0, false);
        assert!(g.logs().is_empty());
        assert_eq!(
            g.params[&Param::Image],
            Value::Texture {
                texture: TEXTURE,
                srgb: linear
            }
        );
        assert_eq!(g.sprites.len(), 1);
        assert_eq!(g.sprites[0].srgb, linear);
        assert_eq!(g.srgb, srgb, "the flag is put back");
        assert!(g.stack.is_empty());
        assert_eq!(g.at, (0.0, 0.0), "the matrix is put back");
    }
}

/// Mirrors `draw_places_and_flips`: the sprite at (x, y), cx by cy, with
/// GS_FLIP_V when flipped.
#[test]
fn test_source_draw_draw_places_and_flips() {
    let cases: [(i32, i32, u32, u32, bool); 12] = [
        (0, 0, 0, 0, false),
        (0, 0, 0, 0, true),
        (3, 2, 0, 0, false),
        (3, 2, 0, 0, true),
        (5, 0, 0, 0, false),
        (0, 6, 0, 0, true),
        (-1, 0, 0, 0, false),
        (0, -1, 0, 0, true),
        (2, 2, 4, 4, false),
        (2, 2, 4, 4, true),
        (0, 0, 4, 2, true),
        (1, 3, 2, 4, false),
    ];
    for (x, y, cx, cy, flip) in cases {
        for linear in [false, true] {
            let mut g = Model::new(true, linear, false);
            source_draw::draw(&mut g, Some(TEXTURE), x, y, cx, cy, flip);
            assert_eq!(
                g.sprites,
                [Sprite {
                    texture: TEXTURE,
                    at: (x as f32, y as f32),
                    flip: if flip { GS_FLIP_V } else { 0 },
                    cx,
                    cy,
                    srgb: linear,
                    image: Some(Value::Texture {
                        texture: TEXTURE,
                        srgb: linear
                    }),
                }]
            );
            assert!(g.stack.is_empty());
        }
    }
}

/// The whole call order, at the origin (no push) and off it.
#[test]
fn edge_draw_call_order() {
    for (x, y) in [(0, 0), (4, 0), (0, -4), (9, 9)] {
        for linear in [false, true] {
            let mut g = Model::new(true, linear, !linear);
            source_draw::draw(&mut g, Some(TEXTURE), x, y, 5, 6, true);
            let mut want = vec![
                Call::CurrentEffect,
                Call::LinearSrgb,
                Call::SrgbEnabled,
                Call::EnableSrgb(linear),
                Call::Param(Param::Image),
                if linear {
                    Call::SetTextureSrgb(Some(Param::Image), TEXTURE)
                } else {
                    Call::SetTexture(Some(Param::Image), TEXTURE)
                },
            ];
            let moved = x != 0 || y != 0;
            if moved {
                want.push(Call::Push);
                want.push(Call::Translate(
                    (x as f32).to_bits(),
                    (y as f32).to_bits(),
                    0.0f32.to_bits(),
                ));
            }
            want.push(Call::Draw {
                texture: TEXTURE,
                flip: GS_FLIP_V,
                cx: 5,
                cy: 6,
            });
            if moved {
                want.push(Call::Pop);
            }
            want.push(Call::EnableSrgb(!linear));
            assert_eq!(g.calls, want, "at ({x}, {y}), linear {linear}");
        }
    }
}

/// Extreme positions translate by the C's `(float)` casts, and a NULL
/// image param is passed on.
#[test]
fn edge_draw_extreme_positions_and_null_param() {
    for (x, y) in [
        (i32::MIN, 0),
        (0, i32::MAX),
        (i32::MAX, i32::MIN),
        (16_777_217, -16_777_217),
    ] {
        let mut g = Model::new(true, false, false);
        g.null_params = true;
        source_draw::draw(&mut g, Some(TEXTURE), x, y, u32::MAX, 0, false);
        assert!(g.calls.contains(&Call::SetTexture(None, TEXTURE)));
        assert!(g.calls.contains(&Call::Translate(
            (x as f32).to_bits(),
            (y as f32).to_bits(),
            0
        )));
        assert!(g.calls.contains(&Call::Draw {
            texture: TEXTURE,
            flip: 0,
            cx: u32::MAX,
            cy: 0
        }));
    }
}
