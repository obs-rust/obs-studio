//! Property construction and settings application, in the original callback order.
pub trait Host {
    type Properties: Default;
    type Settings;
    fn modern(&self) -> bool;
    fn legacy(&self) -> bool;
    fn defaults(&mut self) -> Self::Settings;
    fn construct(&mut self, modern: bool) -> Self::Properties;
    fn apply(&mut self, props: &Self::Properties, defaults: Option<&Self::Settings>);
    fn release(&mut self, settings: Self::Settings);
}

pub fn construct<H: Host>(host: &mut H, type_query: bool) -> H::Properties {
    todo!()
}
