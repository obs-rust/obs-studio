//! Shader parameter validation and synchronous backend dispatch.
pub trait Host {
    type Handle;
    type Name;
    type Info;

    fn has_context(&self) -> bool;
    fn debug(&mut self, function: &'static str, reason: &'static str);
    fn count(&mut self, shader: Self::Handle) -> i32;
    fn by_index(&mut self, shader: Self::Handle, index: u32) -> Option<Self::Handle>;
    fn by_name(&mut self, shader: Self::Handle, name: Self::Name) -> Option<Self::Handle>;
    fn info(&mut self, param: Self::Handle, info: Self::Info);
}

pub fn num_params<H: Host>(_host: &mut H, _shader: Option<H::Handle>) -> i32 {
    todo!("shader parameter introspection port")
}

pub fn param_by_idx<H: Host>(
    _host: &mut H,
    _shader: Option<H::Handle>,
    _index: u32,
) -> Option<H::Handle> {
    todo!("shader parameter introspection port")
}

pub fn param_by_name<H: Host>(
    _host: &mut H,
    _shader: Option<H::Handle>,
    _name: Option<H::Name>,
) -> Option<H::Handle> {
    todo!("shader parameter introspection port")
}

pub fn param_info<H: Host>(_host: &mut H, _param: Option<H::Handle>, _info: Option<H::Info>) {
    todo!("shader parameter introspection port")
}
