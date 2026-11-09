//! Texture creation sequencing without private graphics layouts.
use crate::sprite::MipWarning;

#[derive(Clone, Copy, Debug)]
pub struct Request<D> {
    pub width: u32,
    pub height: u32,
    pub format: i32,
    pub levels: u32,
    pub flags: u32,
    pub cube: bool,
    pub data: Option<D>,
}

pub trait Device<D> {
    type Texture;
    fn has_context(&self) -> bool;
    fn no_context(&mut self, cube: bool);
    fn warning(&mut self, warning: MipWarning);
    fn create(&mut self, request: Request<D>) -> Option<Self::Texture>;
}

pub fn create<D: Copy, H: Device<D>>(_host: &mut H, _request: Request<D>) -> Option<H::Texture> {
    todo!()
}
