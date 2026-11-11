//! Render a canvas through its borrowed embedded view, without extra work.

/// Resolve the canvas's view once and pass it unchanged to the view renderer.
/// The host owns locking, source references and rendering/context effects.
pub fn render<C, V>(canvas: C, view: impl FnOnce(C) -> V, render_view: impl FnOnce(V)) {
    let _ = (canvas, view, render_view);
    todo!("#4700: delegate canvas rendering to its view")
}
