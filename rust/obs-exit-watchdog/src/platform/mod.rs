//! The platform boundary (obs-rust/obs-studio#323): the only place in this
//! crate that selects on the host.
//!
//! - `terminate`: `_exit(2)` on Unix, `TerminateProcess` on Windows.
//! - `images`: the loaded images in load order (#4525): `dl_iterate_phdr` on
//!   Linux, dyld on macOS; elsewhere none yet.
//! - `capture_stack`: another thread's return addresses (#4525): a signal and
//!   `backtrace(3)` on Linux, the suspended thread's registers and frame
//!   pointers on macOS; elsewhere none yet.

cfg_select! {
    target_os = "linux" => {
        #[allow(unsafe_code)] // _exit(2)
        mod unix;
        #[allow(unsafe_code)] // dl_iterate_phdr(3), sigaction(2), pthread_kill(3), backtrace(3)
        mod linux;
        pub use unix::terminate;
        pub use linux::{
            IMAGES_SUPPORTED, STACK_SUPPORTED, Thread, capture_stack, current_thread, images,
            prepare_stack,
        };
    }
    target_os = "macos" => {
        #[allow(unsafe_code)] // _exit(2)
        mod unix;
        #[allow(unsafe_code)] // dyld image list, Mach-O load commands, thread_get_state
        mod macos;
        pub use unix::terminate;
        pub use macos::{
            IMAGES_SUPPORTED, STACK_SUPPORTED, Thread, capture_stack, current_thread, images,
            prepare_stack,
        };
    }
    unix => {
        #[allow(unsafe_code)] // _exit(2)
        mod unix;
        mod unsupported;
        pub use unix::terminate;
        pub use unsupported::{
            IMAGES_SUPPORTED, STACK_SUPPORTED, Thread, capture_stack, current_thread, images,
            prepare_stack,
        };
    }
    windows => {
        #[allow(unsafe_code)] // TerminateProcess
        mod windows;
        mod unsupported;
        pub use windows::terminate;
        pub use unsupported::{
            IMAGES_SUPPORTED, STACK_SUPPORTED, Thread, capture_stack, current_thread, images,
            prepare_stack,
        };
    }
}
