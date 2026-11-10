//! Texture metadata validation and backend dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Property {
    Width,
    Height,
    ColorFormat,
}
impl Property {
    pub fn function(self) -> &'static str {
        match self {
            Self::Width => "gs_texture_get_width",
            Self::Height => "gs_texture_get_height",
            Self::ColorFormat => "gs_texture_get_color_format",
        }
    }
}
pub trait Host<S> {
    fn has_context(&self) -> bool;
    fn debug(&mut self, function: &'static str, reason: &'static str);
    fn dispatch(&mut self, property: Property, texture: S) -> u32;
}
pub fn metadata<S>(host: &mut impl Host<S>, texture: Option<S>, property: Property) -> u32 {
    let _ = (host, texture, property);
    todo!("texture metadata port #4515")
}
