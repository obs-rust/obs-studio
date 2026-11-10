//! Scene construction routing; the host owns source creation and its side effects.

/// Borrowed handles and names remain opaque to the routing core.
pub trait Host {
    type Handle: Copy;
    type Name: Copy;
    fn main_canvas(&self) -> Self::Handle;
    fn public(
        &mut self,
        canvas: Self::Handle,
        id: &'static [u8; 6],
        name: Self::Name,
    ) -> Self::Handle;
    fn private(
        &mut self,
        canvas: Self::Handle,
        id: &'static [u8; 6],
        name: Self::Name,
    ) -> Self::Handle;
}

/// Return the newly created source's scene data, without acquiring another reference.
pub fn create<H: Host>(host: &mut H, name: H::Name, private: bool) -> H::Handle {
    let canvas = host.main_canvas();
    if private {
        host.private(canvas, b"scene\0", name)
    } else {
        host.public(canvas, b"scene\0", name)
    }
}
