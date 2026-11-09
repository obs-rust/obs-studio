//! Scene-item transition serialization, with ownership operations supplied by the host.
/// The host owns all handles; each retained data/source reference is released explicitly.
pub trait Host {
    type Item: Copy;
    type Data: Copy;
    type Source: Copy;
    type Text: Copy;
    fn create_data(&mut self) -> Self::Data;
    fn text(&mut self, data: Self::Data, key: &'static str) -> Self::Text;
    fn nonempty(&self, text: Self::Text) -> bool;
    fn object(&mut self, data: Self::Data, key: &'static str) -> Self::Data;
    fn integer(&mut self, data: Self::Data, key: &'static str) -> i64;
    fn create_source(
        &mut self,
        id: Self::Text,
        name: Self::Text,
        settings: Self::Data,
    ) -> Self::Source;
    fn release_source(&mut self, source: Self::Source);
    fn release_data(&mut self, data: Self::Data);
    fn set_transition(&mut self, item: Self::Item, show: bool, source: Option<Self::Source>);
    fn set_duration(&mut self, item: Self::Item, show: bool, duration: u32);
    fn transition(&mut self, item: Self::Item, show: bool) -> Option<Self::Source>;
    fn duration(&mut self, item: Self::Item, show: bool) -> u32;
    fn source_text(&mut self, source: Self::Source, key: &'static str) -> Self::Text;
    fn settings(&mut self, source: Self::Source) -> Self::Data;
    fn set_text(&mut self, data: Self::Data, key: &'static str, text: Self::Text);
    fn set_object(&mut self, data: Self::Data, key: &'static str, object: Self::Data);
    fn set_integer(&mut self, data: Self::Data, key: &'static str, integer: i64);
}
/// Restore a selected slot; missing item/data leaves both slots untouched.
pub fn load<H: Host>(_host: &mut H, _item: Option<H::Item>, _data: Option<H::Data>, _show: bool) {
    todo!()
}
/// Save the selected slot, retaining the settings object in the result.
pub fn save<H: Host>(_host: &mut H, _item: H::Item, _show: bool) -> H::Data {
    todo!()
}
