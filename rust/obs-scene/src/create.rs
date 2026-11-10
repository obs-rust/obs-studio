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
pub fn create<H: Host>(_host: &mut H, _name: H::Name, _private: bool) -> H::Handle {
    todo!("scene constructors Rust swap")
}
