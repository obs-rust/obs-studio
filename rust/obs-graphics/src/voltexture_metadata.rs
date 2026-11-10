//! Volume texture metadata validation and backend dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Property {
    Width,
    Height,
    Depth,
    ColorFormat,
}
impl Property {
    pub fn function(self) -> &'static str {
        match self {
            Self::Width => "gs_voltexture_get_width",
            Self::Height => "gs_voltexture_get_height",
            Self::Depth => "gs_voltexture_get_depth",
            Self::ColorFormat => "gs_voltexture_get_color_format",
        }
    }
}
pub trait Host<S> {
    fn has_context(&self) -> bool;
    fn debug(&mut self, function: &'static str, reason: &'static str);
    fn dispatch(&mut self, property: Property, voltexture: S) -> u32;
}
pub fn metadata<S>(host: &mut impl Host<S>, voltexture: Option<S>, property: Property) -> u32 {
    if !host.has_context() {
        host.debug(
            property.function(),
            "called while not in a graphics context",
        );
        return 0;
    }
    let Some(voltexture) = voltexture else {
        host.debug(property.function(), "Null 'voltex' parameter");
        return 0;
    };
    host.dispatch(property, voltexture)
}
