//! The platform boundary (obs-rust/obs-studio#323): the only place in this
//! crate that selects on the host. V4L2 is Linux only; elsewhere nothing is
//! signalled or watched.

cfg_select! {
    target_os = "linux" => {
        #[allow(unsafe_code)] // kill(2), getuid(2), inotify(7), poll(2), pipe(2)
        mod linux;
        pub use linux::{ProcessSignals, Watcher, current_uid, owner_uid};
    }
    _ => {
        mod other;
        pub use other::{ProcessSignals, Watcher, current_uid, owner_uid};
    }
}
