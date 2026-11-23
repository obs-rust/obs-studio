// The selected platform modules provide scan roots and OS setup.
cfg_select! {
    target_os = "windows" => {
        #[path = "../ffi/windows.rs"]
        #[allow(unsafe_code)]
        mod host;
    }
    target_os = "macos" => {
        #[path = "../ffi/macos.rs"]
        #[allow(unsafe_code)]
        mod host;
    }
    target_os = "linux" => {
        #[path = "../ffi/linux.rs"]
        #[allow(unsafe_code)]
        mod host;
    }
    _ => { mod unsupported; use unsupported as host; }
}

pub(crate) use host::{is_candidate, metadata_root, prepare_process, search_roots};
