//! Core private-data replacement and retained access. Storage belongs to the host.

/// The existing core storage and data-object operations, without owning either.
pub trait Host {
    type Data: Copy;
    fn private_data(&mut self) -> Self::Data;
    fn clear(&mut self, data: Self::Data);
    fn apply(&mut self, data: Self::Data, settings: Self::Data);
    fn addref(&mut self, data: Self::Data);
}

/// Replace user values in the existing object; defaults and identity survive.
pub fn set<H: Host>(host: &mut H, settings: Option<H::Data>) {
    let data = host.private_data();
    host.clear(data);
    if let Some(settings) = settings {
        let data = host.private_data();
        host.apply(data, settings);
    }
}

/// Return the existing object with exactly one additional reference.
pub fn get<H: Host>(host: &mut H) -> H::Data {
    let data = host.private_data();
    host.addref(data);
    data
}
