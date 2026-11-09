//! Encoder property construction, independent of private C layouts.

/// Borrowed callbacks and settings supplied by an encoder type or instance.
pub trait Host {
    type Settings: Copy;
    type Properties;
    fn null() -> Self::Properties;
    fn has_properties(&self) -> bool;
    fn has_properties2(&self) -> bool;
    fn create_settings(&self) -> Self::Settings;
    fn defaults(&self, settings: Self::Settings);
    fn defaults2(&self, settings: Self::Settings);
    fn settings(&self) -> Self::Settings;
    fn properties(&self, instance: bool) -> Self::Properties;
    fn properties2(&self, instance: bool) -> Self::Properties;
    fn apply(&self, properties: &Self::Properties, settings: Self::Settings);
    fn release(&self, settings: Self::Settings);
}

/// Construct properties using type defaults or the instance's current settings.
pub fn construct<H: Host>(_host: Option<&H>, _instance: bool) -> H::Properties {
    todo!()
}
