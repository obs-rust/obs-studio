//! UI handler installation and per-thread bootstrap marking.
use std::cell::Cell;

thread_local! {
    static IS_UI_THREAD: Cell<bool> = const { Cell::new(false) };
}

/// Borrowed host operations. Installation deliberately takes no lock: the
/// bootstrap dispatch may reenter the public UI queue synchronously.
pub trait Host {
    type Handler;
    fn store(&mut self, handler: Self::Handler);
    fn queue_bootstrap(&mut self);
}

/// Store even a missing handler before asking the UI queue to dispatch.
pub fn install<H: Host>(_host: &mut H, _handler: H::Handler) {
    todo!("port obs_set_ui_task_handler")
}

/// Called only when the queued bootstrap actually executes.
pub fn mark_ui_thread() {
    IS_UI_THREAD.set(true);
}

/// The marker survives handler replacement and clearing, as in C.
pub fn is_ui_thread() -> bool {
    IS_UI_THREAD.get()
}
