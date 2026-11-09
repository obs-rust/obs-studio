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
    let _ = (host, info);
    todo!()
}

pub fn type_properties<H: Host>(host: &H, info: Option<H::Info>) -> Option<H::Props> {
    let _ = (host, info);
    todo!()
}

pub fn properties<H: Host>(
    host: &H,
    output: Option<(H::Info, H::Data, H::Data)>,
) -> Option<H::Props> {
    let _ = (host, output);
    todo!()
}
