//! Output defaults and properties orchestration over opaque host handles.
pub trait Host {
    type Info: Copy;
    type Data: Copy;
    type Props;
    fn create(&self) -> Self::Data;
    fn defaults(&self, info: Self::Info, data: Self::Data);
    fn has_properties(&self, info: Self::Info) -> bool;
    fn properties(&self, info: Self::Info, data: Option<Self::Data>) -> Self::Props;
    fn apply(&self, props: &Self::Props, data: Self::Data);
    fn release(&self, data: Self::Data);
    fn null_output(&self);
}

pub fn defaults<H: Host>(host: &H, info: Option<H::Info>) -> Option<H::Data> {
    let info = info?;
    let data = host.create();
    host.defaults(info, data);
    Some(data)
}

pub fn type_properties<H: Host>(host: &H, info: Option<H::Info>) -> Option<H::Props> {
    let info = info?;
    if !host.has_properties(info) {
        return None;
    }
    let data = defaults(host, Some(info))?;
    let props = host.properties(info, None);
    host.apply(&props, data);
    host.release(data);
    Some(props)
}

pub fn properties<H: Host>(
    host: &H,
    output: Option<(H::Info, H::Data, H::Data)>,
) -> Option<H::Props> {
    let Some((info, data, settings)) = output else {
        host.null_output();
        return None;
    };
    if !host.has_properties(info) {
        return None;
    }
    let props = host.properties(info, Some(data));
    host.apply(&props, settings);
    Some(props)
}
