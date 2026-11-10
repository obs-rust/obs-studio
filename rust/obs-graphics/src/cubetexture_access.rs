//! Cube texture validation and backend ownership dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Size,
    Destroy,
    ColorFormat,
}
impl Operation {
    pub fn function(self) -> &'static str {
        match self {
            Self::Size => "gs_cubetexture_get_size",
            Self::Destroy => "gs_cubetexture_destroy",
            Self::ColorFormat => "gs_cubetexture_get_color_format",
        }
    }
}
pub trait Host<S> {
    fn has_context(&self) -> bool;
    fn debug(&mut self, function: &'static str, reason: &'static str);
    fn dispatch(&mut self, operation: Operation, texture: S) -> u32;
}
pub fn access<S>(_host: &mut impl Host<S>, _texture: Option<S>, _operation: Operation) -> u32 {
    todo!("cube texture access port")
}
