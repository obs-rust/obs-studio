//! GPU acceleration status for the status-bar light (obs-rust/obs-studio#846).
//!
//! [`classify`] is pure: it takes the active adapter name (as libobs logs it
//! in `Loading up OpenGL on adapter ...`), the hardware GPUs detected on the
//! system and optional frame timings, and returns green, yellow or red.
//! Detection reads a sysfs/`/dev/dri` tree under a caller-supplied root, so
//! tests use a fixture tree; [`platform::system_devices`] probes the host.

#![deny(unsafe_code)]

use std::path::Path;
use std::process::Command;

pub mod dxgi;
#[allow(unsafe_code)]
pub mod ffi;
pub mod platform;
pub mod report;

/// The light's color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// The active adapter is a hardware GPU.
    Green,
    /// No GPU on the machine; rendering on a software renderer is expected
    /// (a VM or headless box, #1005).
    Yellow,
    /// A GPU is present but OBS renders in software, graphics init failed,
    /// or frames are behind budget.
    Red,
}

/// PCI vendor of a detected GPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vendor {
    Nvidia,
    Amd,
    Intel,
}

impl Vendor {
    /// Map a PCI vendor ID (`0x10de`, `10de`, ...) to a known GPU vendor.
    pub fn from_pci_id(id: &str) -> Option<Self> {
        let id = id.trim();
        let hex = id
            .strip_prefix("0x")
            .or_else(|| id.strip_prefix("0X"))
            .unwrap_or(id);
        match hex.to_ascii_lowercase().as_str() {
            "10de" => Some(Self::Nvidia),
            "1002" => Some(Self::Amd),
            "8086" => Some(Self::Intel),
            _ => None,
        }
    }
}

/// A hardware GPU found on the system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuDevice {
    pub vendor: Vendor,
    /// PCI device ID, lowercase hex without `0x` (e.g. `2504`), if known.
    pub device_id: Option<String>,
    /// Marketing name, if known (from `nvidia-smi`).
    pub name: Option<String>,
    /// Whether a `/dev/dri/renderD*` node belongs to this device.
    pub render_node: bool,
}

/// Graphics-thread timings over a window, in nanoseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameStats {
    /// Median `obs_graphics_thread` time over the window.
    pub median_frame_ns: u64,
    /// The frame interval (budget), e.g. 33_333_333 at 30 fps.
    pub interval_ns: u64,
}

/// One-second samples [`FrameWindow`] keeps (the last 15 seconds).
pub const FRAME_WINDOW: usize = 15;

/// The last [`FRAME_WINDOW`] one-second frame-time averages
/// (`obs_get_average_frame_time_ns`, sampled once a second).
#[derive(Debug, Clone, Default)]
pub struct FrameWindow {
    samples: std::collections::VecDeque<u64>,
}

impl FrameWindow {
    pub const fn new() -> Self {
        Self {
            samples: std::collections::VecDeque::new(),
        }
    }

    /// Record one sample; 0 (no frame rendered yet) is skipped.
    pub fn push(&mut self, frame_ns: u64) {
        if frame_ns == 0 {
            return;
        }
        if self.samples.len() == FRAME_WINDOW {
            self.samples.pop_front();
        }
        self.samples.push_back(frame_ns);
    }

    pub fn samples(&self) -> Vec<u64> {
        self.samples.iter().copied().collect()
    }

    /// [`FrameStats::from_samples`] over the window.
    pub fn stats(&self, interval_ns: u64) -> Option<FrameStats> {
        FrameStats::from_samples(&self.samples(), interval_ns)
    }
}

/// Whether `adapter` is a graphics API name (`gs_get_device_name()`), not an
/// adapter: the frontend then failed to read the real one (#929, #943).
pub fn is_graphics_api_name(adapter: &str) -> bool {
    matches!(adapter.trim(), "OpenGL" | "Direct3D 11" | "Metal")
}

/// Most log lines [`LogCapture`] keeps.
pub const MAX_LOG_LINES: usize = 10;
/// Longest log line [`LogCapture`] keeps, in characters.
pub const MAX_LOG_LINE_CHARS: usize = 300;

/// The OBS log lines that explain the GPU state (graphics init, the
/// adapter, DRM render nodes, EGL/GLX), for the dialog (#944).
#[derive(Debug, Clone, Default)]
pub struct LogCapture {
    lines: Vec<String>,
}

impl LogCapture {
    pub const fn new() -> Self {
        Self { lines: Vec::new() }
    }

    /// Keep `line` if it is about graphics, once; the last [`MAX_LOG_LINES`].
    pub fn push(&mut self, line: &str) {
        if !is_graphics_log_line(line) {
            return;
        }
        let line: String = line.trim_end().chars().take(MAX_LOG_LINE_CHARS).collect();
        if self.lines.contains(&line) {
            return;
        }
        // The latest lines win: a failure late in startup matters more.
        if self.lines.len() == MAX_LOG_LINES {
            self.lines.remove(0);
        }
        self.lines.push(line);
    }

    pub fn lines(&self) -> Vec<String> {
        self.lines.clone()
    }
}

/// Whether `line` is about graphics init, the adapter, DRM render nodes or
/// the EGL/GLX platforms. The light's own log lines are excluded, so its
/// startup warning is not quoted back (#963 review).
pub fn is_graphics_log_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with("possible cause:")
        || trimmed.starts_with("detected GPU:")
        || line.contains("GPU acceleration:")
    {
        return false;
    }
    const PHRASES: [&str; 5] = [
        "loading up opengl",
        "loading up direct3d",
        "drm render node",
        "failed to initialize video",
        "graphics module",
    ];
    let lower = line.to_ascii_lowercase();
    PHRASES.iter().any(|p| lower.contains(p))
        || line
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .any(is_egl_or_glx_word)
}

/// `EGL`, `EGL_NOT_INITIALIZED`, `libEGL`, `egl`, `eglInitialize`, `GLX...`,
/// `glXMakeCurrent`; not words that merely contain "egl" ("Negligible").
fn is_egl_or_glx_word(word: &str) -> bool {
    let word = word.strip_prefix("lib").unwrap_or(word);
    let api_call = |prefix: &str| {
        word.strip_prefix(prefix).is_some_and(|rest| {
            rest.is_empty() || rest.starts_with(|c: char| c.is_ascii_uppercase())
        })
    };
    word.starts_with("EGL") || word.starts_with("GLX") || api_call("egl") || api_call("glX")
}

/// Fewest one-second samples [`FrameStats::from_samples`] judges a window on.
pub const MIN_FRAME_SAMPLES: usize = 5;

impl FrameStats {
    /// The median of one-second frame-time averages (`obs_get_average_frame_time_ns`
    /// sampled once a second), or `None` with fewer than [`MIN_FRAME_SAMPLES`]
    /// samples or no interval. A median keeps one slow second, such as the
    /// first one after launch, from turning the light red.
    pub fn from_samples(samples: &[u64], interval_ns: u64) -> Option<Self> {
        if interval_ns == 0 || samples.len() < MIN_FRAME_SAMPLES {
            return None;
        }
        Some(Self {
            median_frame_ns: median(samples)?,
            interval_ns,
        })
    }
}

/// The median of `values`, or `None` when it is empty. An even count gives
/// the midpoint of the two middle values, rounded down; `u64::midpoint`
/// computes it without overflowing.
pub fn median(values: &[u64]) -> Option<u64> {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let mid = sorted.len() / 2;
    match sorted.len() {
        0 => None,
        n if n % 2 == 1 => Some(sorted[mid]),
        _ => Some(sorted[mid - 1].midpoint(sorted[mid])),
    }
}

/// Whether `adapter` names a software renderer (llvmpipe, softpipe,
/// Microsoft Basic Render Driver, SwiftShader).
pub fn is_software_renderer(adapter: &str) -> bool {
    let a = adapter.to_ascii_lowercase();
    [
        "llvmpipe",
        "softpipe",
        "microsoft basic render driver",
        "swiftshader",
    ]
    .iter()
    .any(|s| a.contains(s))
}

/// Classify the GPU acceleration state.
///
/// `adapter` is `None` when graphics initialization failed.
pub fn classify(adapter: Option<&str>, devices: &[GpuDevice], frames: Option<FrameStats>) -> State {
    let Some(adapter) = adapter else {
        return State::Red;
    };
    if frames.is_some_and(|f| f.median_frame_ns > f.interval_ns) {
        return State::Red;
    }
    match (is_software_renderer(adapter), devices.is_empty()) {
        (false, _) => State::Green,
        // A GPU OBS can't use is the problem to fix; a machine with no GPU
        // rendering in software is expected (#1005).
        (true, false) => State::Red,
        (true, true) => State::Yellow,
    }
}

/// Detect hardware GPUs from `<root>/sys/class/drm/card*/device/vendor`.
/// A device has a render node when a `<root>/sys/class/drm/renderD*` entry
/// has the same vendor and device ID and `<root>/dev/dri/renderD*` exists.
pub fn detect_devices(root: &Path) -> Vec<GpuDevice> {
    let drm = root.join("sys/class/drm");
    let Ok(entries) = std::fs::read_dir(&drm) else {
        return Vec::new();
    };
    let read_id = |dir: &Path, file: &str| -> Option<String> {
        let raw = std::fs::read_to_string(dir.join("device").join(file)).ok()?;
        let raw = raw.trim();
        Some(raw.strip_prefix("0x").unwrap_or(raw).to_ascii_lowercase())
    };
    let mut cards = Vec::new();
    let mut render_nodes = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let dir = entry.path();
        // `card0` and `renderD128`, not connectors such as `card0-HDMI-A-1`.
        if let Some(n) = name.strip_prefix("card") {
            if n.bytes().all(|b| b.is_ascii_digit()) && !n.is_empty() {
                cards.push(dir);
            }
        } else if name.starts_with("renderD") && root.join("dev/dri").join(&name).exists() {
            render_nodes.push((read_id(&dir, "vendor"), read_id(&dir, "device")));
        }
    }
    cards
        .iter()
        .filter_map(|dir| {
            let vendor_id = read_id(dir, "vendor");
            let vendor = Vendor::from_pci_id(vendor_id.as_deref()?)?;
            let device_id = read_id(dir, "device");
            let render_node = render_nodes.contains(&(vendor_id, device_id.clone()));
            Some(GpuDevice {
                vendor,
                device_id,
                name: None,
                render_node,
            })
        })
        .collect()
}

/// Parse `nvidia-smi --query-gpu=name,pci.device_id --format=csv,noheader`.
///
/// Each line is `<name>, 0x<device><vendor>` (e.g. `NVIDIA GeForce RTX 3060,
/// 0x250410DE`). Lines that do not parse are skipped.
pub fn parse_nvidia_smi(output: &str) -> Vec<GpuDevice> {
    output
        .lines()
        .filter_map(|line| {
            let (name, id) = line.rsplit_once(',')?;
            let id = id.trim();
            let hex = id.strip_prefix("0x").or_else(|| id.strip_prefix("0X"))?;
            if hex.len() != 8 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return None;
            }
            let (device, vendor) = hex.split_at(4);
            Some(GpuDevice {
                vendor: Vendor::from_pci_id(vendor)?,
                device_id: Some(device.to_ascii_lowercase()),
                name: Some(name.trim().to_owned()),
                render_node: false,
            })
        })
        .collect()
}

/// Run `nvidia-smi`; an absent tool or a failing driver yields no devices.
pub fn query_nvidia_smi() -> Vec<GpuDevice> {
    match Command::new("nvidia-smi")
        .args(["--query-gpu=name,pci.device_id", "--format=csv,noheader"])
        .output()
    {
        Ok(out) if out.status.success() => parse_nvidia_smi(&String::from_utf8_lossy(&out.stdout)),
        _ => Vec::new(),
    }
}

/// Add `extra` devices not already in `devices` (same vendor and device ID);
/// a match fills in its name if it has none.
pub fn merge_devices(devices: &mut Vec<GpuDevice>, extra: Vec<GpuDevice>) {
    for e in extra {
        // The same GPU (vendor and device ID) is listed once; a match only
        // fills in a missing name (DXGI names its GPUs, sysfs does not).
        match devices
            .iter_mut()
            .find(|d| d.vendor == e.vendor && d.device_id == e.device_id)
        {
            Some(d) => {
                if d.name.is_none() {
                    d.name = e.name;
                }
            }
            None => devices.push(e),
        }
    }
}

/// The system's GPUs, probed once per process. Probing runs `nvidia-smi`
/// (tens of milliseconds), so the frontend starts it on a thread at launch
/// ([`DeviceCache::prefetch`]) and the first report, half a second later,
/// finds it done instead of waiting for it.
pub struct DeviceCache {
    cell: std::sync::OnceLock<Vec<GpuDevice>>,
    prefetch: std::sync::Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl DeviceCache {
    pub const fn new() -> Self {
        Self {
            cell: std::sync::OnceLock::new(),
            prefetch: std::sync::Mutex::new(None),
        }
    }

    /// The devices, probed with `probe` unless they already were (or a
    /// prefetch is probing them now: then this waits for it).
    pub fn get(&self, probe: impl FnOnce() -> Vec<GpuDevice>) -> Vec<GpuDevice> {
        let pending = self
            .prefetch
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(thread) = pending {
            // A panicking probe leaves the cell empty; `probe` fills it below.
            let _ = thread.join();
        }
        self.cell.get_or_init(probe).clone()
    }

    /// Start probing with `probe` on a new thread and return at once.
    pub fn prefetch(&'static self, probe: fn() -> Vec<GpuDevice>) {
        let mut pending = self.prefetch.lock().unwrap_or_else(|e| e.into_inner());
        if pending.is_some() || self.cell.get().is_some() {
            return;
        }
        let spawned = std::thread::Builder::new()
            .name("gpu-status probe".into())
            .spawn(move || {
                self.cell.get_or_init(probe);
            });
        // Without a thread the first report probes, as before.
        *pending = spawned.ok();
    }
}

impl Default for DeviceCache {
    fn default() -> Self {
        Self::new()
    }
}

/// The PCI device OBS renders on (obs-rust/obs-studio#1020). Utilization
/// sources (NVML, fdinfo, `gpu_busy_percent`) use it to pick the right GPU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderAdapter {
    /// PCI address in sysfs form, e.g. `0000:2b:00.0`.
    Pci(String),
    /// More than one GPU can be the render adapter, so none is picked.
    Ambiguous,
    /// No PCI GPU: software rendering, no sysfs, or a non-PCI device.
    Unknown,
}

/// Resolve the render node that EGL reports for the display
/// (`/dev/dri/renderD128`) to its PCI device under `<root>/sys/class/drm`.
///
/// Without a render node (GLX, or EGL without
/// `EGL_EXT_device_drm_render_node`), a single PCI card is the render
/// adapter; two or more are [`RenderAdapter::Ambiguous`]. A node that is not
/// a `renderD<n>` name, such as `/Software`, is [`RenderAdapter::Unknown`].
pub fn render_adapter(root: &Path, render_node: Option<&str>) -> RenderAdapter {
    let drm = root.join("sys/class/drm");
    if let Some(node) = render_node {
        let name = Path::new(node)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        if !is_drm_name(name, "renderD") {
            return RenderAdapter::Unknown;
        }
        return pci_address_of(&drm.join(name)).map_or(RenderAdapter::Unknown, RenderAdapter::Pci);
    }
    let Ok(entries) = std::fs::read_dir(&drm) else {
        return RenderAdapter::Unknown;
    };
    let mut found: Vec<String> = entries
        .flatten()
        .filter(|e| is_drm_name(&e.file_name().to_string_lossy(), "card"))
        .filter_map(|e| pci_address_of(&e.path()))
        .collect();
    found.sort();
    found.dedup();
    match found.len() {
        0 => RenderAdapter::Unknown,
        1 => RenderAdapter::Pci(found.remove(0)),
        _ => RenderAdapter::Ambiguous,
    }
}

/// `card0` or `renderD128`, not a connector such as `card0-HDMI-A-1`.
fn is_drm_name(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix)
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// The PCI address of a `sys/class/drm` entry: the last component of its
/// `device` link (`../../../0000:2b:00.0`), if that is a PCI address.
fn pci_address_of(entry: &Path) -> Option<String> {
    let target = std::fs::read_link(entry.join("device")).ok()?;
    let name = target.file_name()?.to_str()?;
    is_pci_address(name).then(|| name.to_owned())
}

/// `<domain>:<bus>:<device>.<function>`, e.g. `0000:2b:00.0`. The domain
/// can be longer than four hex digits (Intel VMD uses `10000`).
fn is_pci_address(s: &str) -> bool {
    let hex = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_hexdigit());
    let mut parts = s.splitn(3, ':');
    let (Some(domain), Some(bus), Some(rest)) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let Some((device, function)) = rest.split_once('.') else {
        return false;
    };
    domain.len() >= 4
        && hex(domain)
        && bus.len() == 2
        && hex(bus)
        && device.len() == 2
        && hex(device)
        && matches!(function.as_bytes(), [b'0'..=b'7'])
}
