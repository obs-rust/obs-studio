// The selected loader modules live in ffi/ because they call the host OS and SDK ABI.
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

pub(crate) use host::{
    WINDOWS_COM_ORDER, enumerate, is_candidate, metadata_root, prepare_process, search_roots,
};
