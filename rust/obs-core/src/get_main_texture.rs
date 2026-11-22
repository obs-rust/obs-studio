//! Borrow the main canvas's rendered texture without changing ownership.
/// The main canvas's mix's render texture, or `None` when there is no main
/// canvas, no mix yet (before the first video reset), the mix's texture has
/// not been rendered, or the rendered mix has no texture. The flag is read
/// before the texture, each read borrowed: no reference is acquired and
/// nothing is written.
pub fn get_main_texture<C, M, T>(
    canvas: Option<C>,
    mix: impl FnOnce(C) -> Option<M>,
    rendered: impl FnOnce(&M) -> bool,
    texture: impl FnOnce(M) -> Option<T>,
) -> Option<T> {
    canvas.and_then(mix).filter(rendered).and_then(texture)
}
