//! The platform boundary (obs-rust/obs-studio#323): the only place in this
//! crate that selects on the host.

cfg_select! {
    unix => {
        #[allow(unsafe_code)] // _exit(2)
        mod unix;
        pub use unix::terminate;
    }
    windows => {
        #[allow(unsafe_code)] // TerminateProcess
        mod windows;
        pub use windows::terminate;
    }
}
