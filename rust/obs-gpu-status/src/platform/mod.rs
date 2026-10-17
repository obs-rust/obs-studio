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
        pub fn render_adapter(hint: crate::RenderHint<'_>) -> crate::RenderAdapter {
            crate::render_adapter(std::path::Path::new("/"), hint.render_node)
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

        /// The DXGI adapter that libobs-d3d11 renders on, by LUID
        /// (obs-rust/obs-studio#1020).
        pub fn render_adapter(hint: crate::RenderHint<'_>) -> crate::RenderAdapter {
            crate::dxgi::render_adapter_from_dxgi(hint.adapter_index, &windows::dxgi_adapters())
        }
    }
    target_os = "macos" => {
        #[allow(unsafe_code)] // Metal, IOKit and objc calls; see the module docs
        mod macos;

        /// GPUs on this machine. macOS has no sysfs or DXGI, so only
        /// `nvidia-smi` is consulted.
        pub fn system_devices() -> Vec<GpuDevice> {
            crate::query_nvidia_smi()
        }

        /// The GPU that OBS renders on, by registry ID
        /// (obs-rust/obs-studio#1020). macOS has no render node; both
        /// renderers use Metal's default GPU, so the hint is not needed.
        pub fn render_adapter(_hint: crate::RenderHint<'_>) -> crate::RenderAdapter {
            crate::render_adapter_from_registry(
                macos::default_metal_registry_id(),
                &macos::accelerator_registry_ids(),
            )
        }
    }
    _ => {
        /// GPUs on this machine: only `nvidia-smi` is consulted.
        pub fn system_devices() -> Vec<GpuDevice> {
            crate::query_nvidia_smi()
        }

        /// The render adapter is not known on this platform.
        pub fn render_adapter(_hint: crate::RenderHint<'_>) -> crate::RenderAdapter {
            crate::RenderAdapter::Unknown
        }
    }
}
