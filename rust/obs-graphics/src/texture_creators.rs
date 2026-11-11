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

pub fn create<D: Copy, H: Device<D>>(host: &mut H, mut request: Request<D>) -> Option<H::Texture> {
    if !host.has_context() {
        host.no_context(request.cube);
        return None;
    }
    let plan =
        crate::sprite::texture_flags(request.width, request.height, request.flags, request.levels);
    if plan.warning != MipWarning::Ok {
        host.warning(plan.warning);
    }
    if request.cube && plan.warning == MipWarning::RenderTarget {
        request.data = None;
    }
    request.flags = plan.flags;
    request.levels = plan.levels;
    host.create(request)
}
