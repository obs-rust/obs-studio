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

fn validate<H: Host>(
    host: &mut H,
    function: &'static str,
    handle: Option<H::Handle>,
    reason: &'static str,
) -> Option<H::Handle> {
    if !host.has_context() {
        host.debug(function, "called while not in a graphics context");
        return None;
    }
    if handle.is_none() {
        host.debug(function, reason);
    }
    handle
}

pub fn num_params<H: Host>(host: &mut H, shader: Option<H::Handle>) -> i32 {
    let Some(shader) = validate(
        host,
        "gs_shader_get_num_params",
        shader,
        "Null 'shader' parameter",
    ) else {
        return 0;
    };
    host.count(shader)
}

pub fn param_by_idx<H: Host>(
    host: &mut H,
    shader: Option<H::Handle>,
    index: u32,
) -> Option<H::Handle> {
    let shader = validate(
        host,
        "gs_shader_get_param_by_idx",
        shader,
        "Null 'shader' parameter",
    )?;
    host.by_index(shader, index)
}

pub fn param_by_name<H: Host>(
    host: &mut H,
    shader: Option<H::Handle>,
    name: Option<H::Name>,
) -> Option<H::Handle> {
    let function = "gs_shader_get_param_by_name";
    let shader = validate(host, function, shader, "Null 'shader' parameter")?;
    let Some(name) = name else {
        host.debug(function, "Null 'name' parameter");
        return None;
    };
    host.by_name(shader, name)
}

pub fn param_info<H: Host>(host: &mut H, param: Option<H::Handle>, info: Option<H::Info>) {
    let function = "gs_shader_get_param_info";
    let Some(param) = validate(host, function, param, "Null 'param' parameter") else {
        return;
    };
    let Some(info) = info else {
        host.debug(function, "Null 'info' parameter");
        return;
    };
    host.info(param, info);
}
