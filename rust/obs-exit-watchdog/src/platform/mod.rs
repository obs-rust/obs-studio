//! The platform boundary (obs-rust/obs-studio#323): the only place in this
//! crate that selects on the host.
//!
//! - `terminate`: `_exit(2)` on Unix, `TerminateProcess` on Windows.
//! - `images`: the loaded images in load order (#4525): `dl_iterate_phdr` on
//!   Linux, dyld on macOS, Toolhelp on Windows (#4589); elsewhere none yet.
//! - `capture_stack`: another thread's return addresses (#4525): a signal and
//!   `backtrace(3)` on Linux, the suspended thread's registers and frame
//!   pointers on macOS, its registers and the unwind data on Windows x64
//!   (#4589); elsewhere none yet.
//! - the watchdog process (#4589): `spawn_helper`, `arm_helper`,
//!   `helper_wait` and `terminate_parent` on Windows; elsewhere none.

cfg_select! {
    target_os = "linux" => {
        #[allow(unsafe_code)] // _exit(2)
        mod unix;
        #[allow(unsafe_code)] // dl_iterate_phdr(3), sigaction(2), pthread_kill(3), backtrace(3)
        mod linux;
        pub use unix::terminate;
        mod no_helper;
        pub use no_helper::{Helper, Wait, arm_helper, helper_wait, spawn_helper, terminate_parent};
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
        mod no_helper;
        pub use no_helper::{Helper, Wait, arm_helper, helper_wait, spawn_helper, terminate_parent};
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
        mod no_helper;
        pub use no_helper::{Helper, Wait, arm_helper, helper_wait, spawn_helper, terminate_parent};
        pub use unsupported::{
            IMAGES_SUPPORTED, STACK_SUPPORTED, Thread, capture_stack, current_thread, images,
            prepare_stack,
        };
    }
    all(windows, target_arch = "x86_64") => {
        #[allow(unsafe_code)] // TerminateProcess
        mod windows;
        pub use windows::terminate;
        #[allow(unsafe_code)] // CreateProcessW, DuplicateHandle, events, waits
        mod windows_helper;
        pub use windows_helper::{Helper, Wait, arm_helper, helper_wait, spawn_helper, terminate_parent};
        #[allow(unsafe_code)] // Toolhelp module snapshot
        mod windows_images;
        pub use windows_images::{IMAGES_SUPPORTED, images};
        #[allow(unsafe_code)] // SuspendThread, GetThreadContext, RtlVirtualUnwind
        mod windows_stack;
        pub use windows_stack::{STACK_SUPPORTED, Thread, capture_stack, current_thread, prepare_stack};
    }
    windows => {
        // arm64: the image list; the stack capture (an arm64 CONTEXT and its
        // unwind) is not built yet.
        #[allow(unsafe_code)] // TerminateProcess
        mod windows;
        #[allow(dead_code)] // only its stack half is used here
        mod unsupported;
        pub use windows::terminate;
        #[allow(unsafe_code)] // CreateProcessW, DuplicateHandle, events, waits
        mod windows_helper;
        pub use windows_helper::{Helper, Wait, arm_helper, helper_wait, spawn_helper, terminate_parent};
        #[allow(unsafe_code)] // Toolhelp module snapshot
        mod windows_images;
        pub use windows_images::{IMAGES_SUPPORTED, images};
        pub use unsupported::{STACK_SUPPORTED, Thread, capture_stack, current_thread, prepare_stack};
    }
}
