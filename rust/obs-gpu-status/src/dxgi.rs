//! Windows GPU detection from DXGI adapter descriptions
//! (obs-rust/obs-studio#942). The mapping is pure so it is tested on every
//! host; `platform::windows` fills [`AdapterDesc`] from `EnumAdapters1`.

use crate::{GpuDevice, RenderAdapter, Vendor};

/// `DXGI_ADAPTER_FLAG_SOFTWARE`: WARP and other software adapters.
pub const DXGI_ADAPTER_FLAG_SOFTWARE: u32 = 2;

/// PCI vendor ID Microsoft uses for its software and virtual adapters
/// (Microsoft Basic Render Driver, Hyper-V video).
pub const MICROSOFT_VENDOR_ID: u32 = 0x1414;

/// The fields of `DXGI_ADAPTER_DESC1` the light needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterDesc {
    pub description: String,
    pub vendor_id: u32,
    pub device_id: u32,
    pub flags: u32,
    /// `AdapterLuid` as one number: `HighPart` in the upper 32 bits.
    pub luid: u64,
}

/// The adapter libobs-d3d11 renders on: `EnumAdapters1(index)`, where
/// `index` is `obs_video_info.adapter` (obs-rust/obs-studio#1020).
///
/// An index on a software or Microsoft adapter (WARP, a VM's basic display)
/// or past the list is [`RenderAdapter::Unknown`]. Without an index, one
/// hardware adapter is the answer and two or more are
/// [`RenderAdapter::Ambiguous`].
pub fn render_adapter_from_dxgi(index: Option<u32>, adapters: &[AdapterDesc]) -> RenderAdapter {
    if let Some(index) = index {
        return match adapters.get(index as usize) {
            Some(a) if is_hardware(a) => RenderAdapter::Luid(a.luid),
            _ => RenderAdapter::Unknown,
        };
    }
    let mut luids: Vec<u64> = adapters
        .iter()
        .filter(|a| is_hardware(a))
        .map(|a| a.luid)
        .collect();
    luids.sort_unstable();
    luids.dedup();
    match luids.as_slice() {
        [] => RenderAdapter::Unknown,
        [luid] => RenderAdapter::Luid(*luid),
        _ => RenderAdapter::Ambiguous,
    }
}

/// A GPU, as [`devices_from_dxgi`] counts them: not a software adapter,
/// not one of Microsoft's own, and from a known GPU vendor.
fn is_hardware(a: &AdapterDesc) -> bool {
    a.flags & DXGI_ADAPTER_FLAG_SOFTWARE == 0
        && a.vendor_id != MICROSOFT_VENDOR_ID
        && vendor(a).is_some()
}

fn vendor(a: &AdapterDesc) -> Option<Vendor> {
    Vendor::from_pci_id(&format!("{:04x}", a.vendor_id))
}

/// Hardware GPUs among `adapters`: software adapters and Microsoft's own
/// are dropped, unknown vendors too.
pub fn devices_from_dxgi(adapters: &[AdapterDesc]) -> Vec<GpuDevice> {
    adapters
        .iter()
        .filter(|a| is_hardware(a))
        .filter_map(|a| {
            Some(GpuDevice {
                vendor: vendor(a)?,
                device_id: Some(format!("{:04x}", a.device_id)),
                name: Some(a.description.trim_end_matches(['\0', ' ']).to_owned()),
                // DXGI only lists adapters Windows can render on.
                render_node: true,
            })
        })
        .collect()
}
