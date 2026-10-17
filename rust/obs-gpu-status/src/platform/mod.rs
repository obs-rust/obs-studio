//! The platform boundary (obs-rust/obs-studio#323): the only place in this
//! crate that selects on the host.

use crate::GpuDevice;

cfg_select! {
    target_os = "linux" => {
        /// GPUs on this machine: the sysfs/`/dev/dri` tree, then `nvidia-smi`.
        pub fn system_devices() -> Vec<GpuDevice> {
            let mut devices = crate::detect_devices(std::path::Path::new("/"));
            crate::merge_devices(&mut devices, crate::query_nvidia_smi());
            devices
        }

        /// The PCI device behind the render node that EGL reports for the
        /// display (obs-rust/obs-studio#1020).
        pub fn render_adapter(render_node: Option<&str>) -> crate::RenderAdapter {
            crate::render_adapter(std::path::Path::new("/"), render_node)
        }
    }
    windows => {
        #[allow(unsafe_code)] // DXGI COM calls; see the module docs
        mod windows;

        /// GPUs on this machine: DXGI's hardware adapters, then `nvidia-smi`
        /// (obs-rust/obs-studio#942).
        pub fn system_devices() -> Vec<GpuDevice> {
            let mut devices = crate::dxgi::devices_from_dxgi(&windows::dxgi_adapters());
            crate::merge_devices(&mut devices, crate::query_nvidia_smi());
            devices
        }

        /// The render adapter. The DXGI LUID part of
        /// obs-rust/obs-studio#1020 is open, so this is always unknown.
        pub fn render_adapter(_render_node: Option<&str>) -> crate::RenderAdapter {
            crate::RenderAdapter::Unknown
        }
    }
    _ => {
        /// GPUs on this machine. macOS has no sysfs or DXGI, so only
        /// `nvidia-smi` is consulted.
        pub fn system_devices() -> Vec<GpuDevice> {
            crate::query_nvidia_smi()
        }

        /// The render adapter. The Metal `registryID` part of
        /// obs-rust/obs-studio#1020 is open, so this is always unknown.
        pub fn render_adapter(_render_node: Option<&str>) -> crate::RenderAdapter {
            crate::RenderAdapter::Unknown
        }
    }
}
