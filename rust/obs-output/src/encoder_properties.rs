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
pub fn construct<H: Host>(host: Option<&H>, instance: bool) -> H::Properties {
    let Some(host) = host else {
        return H::null();
    };
    if !host.has_properties2() && !host.has_properties() {
        return H::null();
    }
    let settings = if instance {
        host.settings()
    } else {
        let settings = host.create_settings();
        host.defaults(settings);
        host.defaults2(settings);
        settings
    };
    let properties = if host.has_properties2() {
        host.properties2(instance)
    } else {
        host.properties(instance)
    };
    host.apply(&properties, settings);
    if !instance {
        host.release(settings);
    }
    properties
}
