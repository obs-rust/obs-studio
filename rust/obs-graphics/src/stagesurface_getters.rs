//! Stage-surface metadata validation and backend dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Property {
    Width,
    Height,
    ColorFormat,
}
impl Property {
    pub fn function(self) -> &'static str {
        match self {
            Self::Width => "gs_stagesurface_get_width",
            Self::Height => "gs_stagesurface_get_height",
            Self::ColorFormat => "gs_stagesurface_get_color_format",
        }
    }
}
pub trait Host<S> {
    fn has_context(&self) -> bool;
    fn debug(&mut self, function: &'static str, reason: &'static str);
    fn dispatch(&mut self, property: Property, surface: S) -> u32;
}
pub fn metadata<S>(host: &mut impl Host<S>, surface: Option<S>, property: Property) -> u32 {
    if !host.has_context() {
        host.debug(
            property.function(),
            "called while not in a graphics context",
        );
        return 0;
    }
    let Some(surface) = surface else {
        host.debug(property.function(), "Null 'stagesurf' parameter");
        return 0;
    };
    host.dispatch(property, surface)
}
