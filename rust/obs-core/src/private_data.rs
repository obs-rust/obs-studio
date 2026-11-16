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
pub fn set<H: Host>(_host: &mut H, _settings: Option<H::Data>) {
    todo!()
}

/// Return the existing object with exactly one additional reference.
pub fn get<H: Host>(_host: &mut H) -> H::Data {
    todo!()
}
