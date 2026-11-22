//! Safe mirrors of test_get_main_texture.c, plus the NULL main canvas the C
//! test cannot reach (the core global is hidden from test executables).
use obs_core::get_main_texture::get_main_texture;
use std::cell::Cell;

#[derive(Debug, PartialEq, Eq)]
struct Texture(u8);

#[derive(Debug, Clone, PartialEq, Eq)]
struct Mix<'a> {
    render_texture: Cell<Option<&'a Texture>>,
    texture_rendered: Cell<bool>,
}

impl<'a> Mix<'a> {
    fn new(texture: Option<&'a Texture>, rendered: bool) -> Self {
        Mix {
            render_texture: Cell::new(texture),
            texture_rendered: Cell::new(rendered),
        }
    }
}

struct Canvas<'a> {
    mix: Cell<Option<&'a Mix<'a>>>,
    refs: Cell<i64>,
    weak_refs: Cell<i64>,
}

impl<'a> Canvas<'a> {
    fn new() -> Self {
        Canvas {
            mix: Cell::new(None),
            refs: Cell::new(1),
            weak_refs: Cell::new(1),
        }
    }

    fn refs(&self) -> (i64, i64) {
        (self.refs.get(), self.weak_refs.get())
    }
}

/// `obs_get_main_texture()` over the safe core, reading the main canvas's
/// fields the way the C adapters do: borrowed, with no reference taken.
fn obs_get_main_texture<'a>(main: Option<&'a Canvas<'a>>) -> Option<&'a Texture> {
    get_main_texture(
        main,
        |canvas| canvas.mix.get(),
        |mix| mix.texture_rendered.get(),
        |mix| mix.render_texture.get(),
    )
}

static TEXTURE_A: Texture = Texture(1);
static TEXTURE_B: Texture = Texture(2);

fn is(found: Option<&Texture>, expected: &Texture) -> bool {
    found.is_some_and(|found| core::ptr::eq(found, expected))
}

#[test]
fn main_canvas_without_mix_returns_null() {
    let canvas = Canvas::new();
    let before = canvas.refs();
    for _ in 0..16 {
        assert!(obs_get_main_texture(Some(&canvas)).is_none());
    }
    assert!(canvas.mix.get().is_none());
    assert_eq!(canvas.refs(), before);
}

#[test]
fn unrendered_mix_returns_null() {
    let canvas = Canvas::new();
    assert!(canvas.mix.get().is_none());
    let mix = Mix::new(Some(&TEXTURE_A), false);
    canvas.mix.set(Some(&mix));
    let refs = canvas.refs();

    // A texture that exists but has not been rendered this frame is not
    // handed out.
    let before = mix.clone();
    for _ in 0..16 {
        assert!(obs_get_main_texture(Some(&canvas)).is_none());
    }
    assert_eq!(mix, before);
    assert!(core::ptr::eq(canvas.mix.get().unwrap(), &mix));
    assert_eq!(canvas.refs(), refs);
}

#[test]
fn rendered_mix_texture_is_borrowed_by_identity() {
    let canvas = Canvas::new();
    assert!(canvas.mix.get().is_none());
    let mix = Mix::new(Some(&TEXTURE_A), true);
    canvas.mix.set(Some(&mix));
    let refs = canvas.refs();

    let before = mix.clone();
    for _ in 0..16 {
        assert!(is(obs_get_main_texture(Some(&canvas)), &TEXTURE_A));
    }
    assert_eq!(mix, before);
    assert!(core::ptr::eq(canvas.mix.get().unwrap(), &mix));
    assert_eq!(canvas.refs(), refs);

    // The getter follows the current mix's texture, not a cached value.
    mix.render_texture.set(Some(&TEXTURE_B));
    let before = mix.clone();
    assert!(is(obs_get_main_texture(Some(&canvas)), &TEXTURE_B));
    assert_eq!(mix, before);

    // A rendered mix with no texture answers NULL, not the stale one.
    mix.render_texture.set(None);
    assert!(obs_get_main_texture(Some(&canvas)).is_none());

    // Clearing the flag withdraws the texture again.
    mix.render_texture.set(Some(&TEXTURE_A));
    mix.texture_rendered.set(false);
    assert!(obs_get_main_texture(Some(&canvas)).is_none());
    mix.texture_rendered.set(true);
    assert!(is(obs_get_main_texture(Some(&canvas)), &TEXTURE_A));
    assert!(core::ptr::eq(canvas.mix.get().unwrap(), &mix));
    assert_eq!(canvas.refs(), refs);

    canvas.mix.set(None);
    assert!(obs_get_main_texture(Some(&canvas)).is_none());
}

#[test]
fn auxiliary_canvas_mix_is_not_the_main_texture() {
    let main = Canvas::new();
    let other = Canvas::new();
    let mix = Mix::new(Some(&TEXTURE_B), true);
    other.mix.set(Some(&mix));
    assert!(main.mix.get().is_none());
    assert!(obs_get_main_texture(Some(&main)).is_none());
}

#[test]
fn null_main_canvas_returns_null_without_reading_further() {
    let texture = get_main_texture(
        None::<&Canvas>,
        |_| -> Option<&Mix> { panic!("no canvas to read a mix from") },
        |_| -> bool { panic!("no mix to read a flag from") },
        |_| -> Option<&Texture> { panic!("no mix to read a texture from") },
    );
    assert!(texture.is_none());
}

#[test]
fn missing_mix_never_reads_the_flag_or_texture() {
    let canvas = Canvas::new();
    let texture = get_main_texture(
        Some(&canvas),
        |canvas| canvas.mix.get(),
        |_| -> bool { panic!("no mix to read a flag from") },
        |_| -> Option<&Texture> { panic!("no mix to read a texture from") },
    );
    assert!(texture.is_none());
}

#[test]
fn unrendered_mix_never_reads_the_texture() {
    let mix = Mix::new(Some(&TEXTURE_A), false);
    let texture = get_main_texture(
        Some(&mix),
        Some,
        |mix| mix.texture_rendered.get(),
        |_| -> Option<&Texture> { panic!("an unrendered texture is not read") },
    );
    assert!(texture.is_none());
}
