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
    if !host.has_context() {
        host.debug(
            property.function(),
            "called while not in a graphics context",
        );
        return 0;
    }
    let Some(texture) = texture else {
        host.debug(property.function(), "Null 'tex' parameter");
        return 0;
    };
    host.dispatch(property, texture)
}
