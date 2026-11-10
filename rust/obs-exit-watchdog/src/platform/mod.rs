//! The platform boundary (obs-rust/obs-studio#323): the only place in this
//! crate that selects on the host.
//!
//! - `terminate`: `_exit(2)` on Unix, `TerminateProcess` on Windows.
//! - `images`: the loaded images in load order (#4525): `dl_iterate_phdr` on
//!   Linux, dyld on macOS; elsewhere none yet.

cfg_select! {
    target_os = "linux" => {
        #[allow(unsafe_code)] // _exit(2)
        mod unix;
        #[allow(unsafe_code)] // dl_iterate_phdr(3)
        mod linux;
        pub use unix::terminate;
        pub use linux::{IMAGES_SUPPORTED, images};
    }
    target_os = "macos" => {
        #[allow(unsafe_code)] // _exit(2)
        mod unix;
        #[allow(unsafe_code)] // dyld image list, Mach-O load commands
        mod macos;
        pub use unix::terminate;
        pub use macos::{IMAGES_SUPPORTED, images};
    }
    unix => {
        #[allow(unsafe_code)] // _exit(2)
        mod unix;
        mod no_images;
        pub use unix::terminate;
        pub use no_images::{IMAGES_SUPPORTED, images};
    }
    windows => {
        #[allow(unsafe_code)] // TerminateProcess
        mod windows;
        mod no_images;
        pub use windows::terminate;
        pub use no_images::{IMAGES_SUPPORTED, images};
    }
}
