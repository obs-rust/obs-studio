//! The platform boundary (obs-rust/obs-studio#323): the only place in this
//! crate that selects on the host.

use crate::nvml::Nvml;
use crate::{GpuDevice, Utilization};

#[allow(unsafe_code)] // libloading; see the module docs
mod nvml_lib;

/// NVML, loaded once per process (obs-rust/obs-studio#1021); `None` without
/// an NVIDIA driver.
pub fn nvml() -> Option<&'static Nvml> {
    static NVML: std::sync::OnceLock<Option<Nvml>> = std::sync::OnceLock::new();
    NVML.get_or_init(|| nvml_lib::load(nvml_candidates()))
        .as_ref()
}

/// Utilization of the GPU OBS renders on, through NVML only: other GPUs
/// are [`Utilization::Unavailable`] here. The other sources (amdgpu #1024,
/// fdinfo #1023, PDH #1022) are not wired in; the frontend consumer
/// (#1025) picks the source per adapter.
pub fn gpu_utilization(hint: crate::RenderHint<'_>) -> Utilization {
    crate::nvml::utilization(nvml(), &render_adapter(hint))
}

/// OBS's own share of the render GPU since the previous call (the
/// tooltip). NVIDIA only so far, like [`gpu_utilization`].
pub fn obs_gpu_share(hint: crate::RenderHint<'_>) -> Option<u32> {
    nvml()?.process_utilization(&render_adapter(hint), std::process::id())
}

/// The utilization sources for the `GPU: x%` label (#1025), in the order
/// [`crate::usage::Sampler`] asks them: NVML, then amdgpu's
/// `gpu_busy_percent` on Linux, IOKit's `Device Utilization %` on macOS
/// (#1026) and the PDH `GPU Engine` counters on Windows (#1022), which read
/// every vendor by LUID.
pub fn usage_sources() -> Vec<Box<dyn crate::usage::UsageSource>> {
    let mut sources: Vec<Box<dyn crate::usage::UsageSource>> = vec![Box::new(NvmlSource)];
    if cfg!(target_os = "linux") {
        sources.push(Box::new(AmdgpuSource));
    }
    if cfg!(target_os = "macos") {
        sources.push(Box::new(IokitSource));
    }
    if cfg!(windows) {
        sources.push(Box::new(PdhSource::new()));
    }
    sources
}

/// NVML: whole GPU.
struct NvmlSource;

impl crate::usage::UsageSource for NvmlSource {
    fn source(&self) -> crate::usage::Source {
        crate::usage::Source::Nvml
    }

    fn whole(&mut self, adapter: &crate::RenderAdapter) -> Utilization {
        crate::nvml::utilization(nvml(), adapter)
    }
}

/// amdgpu's `gpu_busy_percent` under `/sys`: whole GPU only.
struct AmdgpuSource;

impl crate::usage::UsageSource for AmdgpuSource {
    fn source(&self) -> crate::usage::Source {
        crate::usage::Source::Amdgpu
    }

    fn whole(&mut self, adapter: &crate::RenderAdapter) -> Utilization {
        match adapter {
            crate::RenderAdapter::Pci(pci) => {
                crate::amdgpu_busy_percent(std::path::Path::new("/"), pci)
            }
            _ => Utilization::Unavailable,
        }
    }
}

/// IOKit's `Device Utilization %` of the render GPU's `IOAccelerator`
/// (#1026): whole GPU only.
struct IokitSource;

impl crate::usage::UsageSource for IokitSource {
    fn source(&self) -> crate::usage::Source {
        crate::usage::Source::Iokit
    }

    fn whole(&mut self, adapter: &crate::RenderAdapter) -> Utilization {
        match adapter {
            crate::RenderAdapter::RegistryId(id) => {
                crate::iokit::device_utilization(&accelerator_statistics(*id))
            }
            _ => Utilization::Unavailable,
        }
    }
}

/// The PDH `GPU Engine` counters (#1022): whole GPU only. The query stays
/// open between ticks, so a tick is one collection, averaged since the
/// previous tick.
struct PdhSource(crate::pdh::PdhTicker);

impl PdhSource {
    fn new() -> Self {
        Self(crate::pdh::PdhTicker::new(Box::new(open_engine_counters)))
    }
}

impl crate::usage::UsageSource for PdhSource {
    fn source(&self) -> crate::usage::Source {
        crate::usage::Source::Pdh
    }

    fn whole(&mut self, adapter: &crate::RenderAdapter) -> Utilization {
        match adapter {
            crate::RenderAdapter::Luid(luid) => self.0.utilization(*luid),
            _ => Utilization::Unavailable,
        }
    }
}

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

        fn nvml_candidates() -> &'static [&'static str] {
            crate::nvml::LINUX_CANDIDATES
        }

        /// PDH `GPU Engine` counters exist only on Windows.
        pub fn gpu_engine_samples() -> Vec<(String, f64)> {
            Vec::new()
        }

        /// PDH `GPU Engine` counters exist only on Windows.
        fn open_engine_counters() -> Option<Box<dyn crate::pdh::EngineCounters>> {
            None
        }

        /// `IOAccelerator` statistics exist only on macOS.
        fn accelerator_statistics(_registry_id: u64) -> Vec<(String, i64)> {
            Vec::new()
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

        fn nvml_candidates() -> &'static [&'static str] {
            crate::nvml::WINDOWS_CANDIDATES
        }

        /// One sample of the PDH `GPU Engine` utilization counters
        /// (obs-rust/obs-studio#1022), for [`crate::pdh::pdh_utilization`].
        /// Blocks about 100 ms.
        pub fn gpu_engine_samples() -> Vec<(String, f64)> {
            windows::gpu_engine_samples()
        }

        /// An open PDH query for the label (#1022).
        fn open_engine_counters() -> Option<Box<dyn crate::pdh::EngineCounters>> {
            windows::EngineQuery::open()
                .map(|q| Box::new(q) as Box<dyn crate::pdh::EngineCounters>)
        }

        /// `IOAccelerator` statistics exist only on macOS.
        fn accelerator_statistics(_registry_id: u64) -> Vec<(String, i64)> {
            Vec::new()
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

        /// Macs have no NVIDIA driver.
        fn nvml_candidates() -> &'static [&'static str] {
            &[]
        }

        /// PDH `GPU Engine` counters exist only on Windows.
        pub fn gpu_engine_samples() -> Vec<(String, f64)> {
            Vec::new()
        }

        /// PDH `GPU Engine` counters exist only on Windows.
        fn open_engine_counters() -> Option<Box<dyn crate::pdh::EngineCounters>> {
            None
        }

        /// The render GPU's `PerformanceStatistics` numbers (#1026).
        fn accelerator_statistics(registry_id: u64) -> Vec<(String, i64)> {
            macos::accelerator_statistics(registry_id)
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

        fn nvml_candidates() -> &'static [&'static str] {
            &[]
        }

        /// PDH `GPU Engine` counters exist only on Windows.
        pub fn gpu_engine_samples() -> Vec<(String, f64)> {
            Vec::new()
        }

        /// PDH `GPU Engine` counters exist only on Windows.
        fn open_engine_counters() -> Option<Box<dyn crate::pdh::EngineCounters>> {
            None
        }

        /// `IOAccelerator` statistics exist only on macOS.
        fn accelerator_statistics(_registry_id: u64) -> Vec<(String, i64)> {
            Vec::new()
        }
    }
}
