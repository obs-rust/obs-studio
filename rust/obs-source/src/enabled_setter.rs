//! Store the enabled flag before dispatching the synchronous enable signal.

pub trait Source {
    fn store_enabled(&self, enabled: bool);
    fn signal_enable(&self, enabled: bool);
}

pub fn set_enabled(source: Option<&impl Source>, enabled: bool, log_null: impl FnOnce()) {
    let Some(source) = source else {
        log_null();
        return;
    };
    source.store_enabled(enabled);
    source.signal_enable(enabled);
}
