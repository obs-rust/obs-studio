//! Windows GPU detection from DXGI adapters (obs-rust/obs-studio#942).

use obs_gpu_status::dxgi::{
    AdapterDesc, DXGI_ADAPTER_FLAG_SOFTWARE, devices_from_dxgi, render_adapter_from_dxgi,
};
use obs_gpu_status::{GpuDevice, RenderAdapter, State, Vendor, classify};

fn adapter(description: &str, vendor_id: u32, device_id: u32, flags: u32) -> AdapterDesc {
    adapter_luid(description, vendor_id, device_id, flags, 0)
}

fn adapter_luid(
    description: &str,
    vendor_id: u32,
    device_id: u32,
    flags: u32,
    luid: u64,
) -> AdapterDesc {
    AdapterDesc {
        description: description.into(),
        vendor_id,
        device_id,
        flags,
        luid,
    }
}

// LUIDs as Windows assigns them at boot: HighPart 0, LowPart a small number.
const LUID_A: u64 = 0x0000_0000_0000_d2a1;
const LUID_B: u64 = 0x0000_0000_0000_e5f3;
const LUID_WARP: u64 = 0x0000_0000_0000_e4c1;

fn warp() -> AdapterDesc {
    adapter_luid(
        "Microsoft Basic Render Driver",
        0x1414,
        0x8c,
        DXGI_ADAPTER_FLAG_SOFTWARE,
        LUID_WARP,
    )
}

#[test]
fn adapter_index_picks_its_own_gpu_among_identical_gpus() {
    // obs_video_info.adapter is the EnumAdapters1 index libobs-d3d11 uses.
    let adapters = [
        adapter_luid("NVIDIA GeForce RTX 3060", 0x10de, 0x2504, 0, LUID_A),
        adapter_luid("NVIDIA GeForce RTX 3060", 0x10de, 0x2504, 0, LUID_B),
        warp(),
    ];
    assert_eq!(
        render_adapter_from_dxgi(Some(1), &adapters),
        RenderAdapter::Luid(LUID_B)
    );
    assert_eq!(
        render_adapter_from_dxgi(Some(0), &adapters),
        RenderAdapter::Luid(LUID_A)
    );
}

#[test]
fn software_or_missing_adapter_index_is_unknown() {
    let adapters = [
        adapter_luid("Intel(R) UHD Graphics 770", 0x8086, 0x4680, 0, LUID_A),
        warp(),
    ];
    assert_eq!(
        render_adapter_from_dxgi(Some(1), &adapters),
        RenderAdapter::Unknown
    );
    assert_eq!(
        render_adapter_from_dxgi(Some(7), &adapters),
        RenderAdapter::Unknown
    );
}

#[test]
fn no_index_with_one_gpu_is_that_gpu() {
    let adapters = [
        adapter_luid("AMD Radeon RX 6600", 0x1002, 0x73ff, 0, LUID_A),
        warp(),
    ];
    assert_eq!(
        render_adapter_from_dxgi(None, &adapters),
        RenderAdapter::Luid(LUID_A)
    );
}

#[test]
fn no_index_with_two_gpus_is_ambiguous() {
    let adapters = [
        adapter_luid("Intel(R) UHD Graphics 770", 0x8086, 0x4680, 0, LUID_A),
        adapter_luid("NVIDIA GeForce RTX 3060", 0x10de, 0x2504, 0, LUID_B),
    ];
    assert_eq!(
        render_adapter_from_dxgi(None, &adapters),
        RenderAdapter::Ambiguous
    );
}

#[test]
fn no_index_on_a_vm_without_gpu_is_unknown() {
    // A VM: Hyper-V video (or QEMU's basic display) and WARP only.
    let adapters = [
        adapter_luid("Microsoft Hyper-V Video", 0x1414, 0x5353, 0, LUID_A),
        warp(),
    ];
    assert_eq!(
        render_adapter_from_dxgi(None, &adapters),
        RenderAdapter::Unknown
    );
    assert_eq!(
        render_adapter_from_dxgi(Some(0), &adapters),
        RenderAdapter::Unknown
    );
}

#[test]
fn unknown_vendor_is_not_a_render_gpu() {
    // As in devices_from_dxgi (#942): a virtio display adapter is not a GPU,
    // so it neither is the answer nor makes a single GPU ambiguous.
    let virtio = adapter_luid("Virtio GPU", 0x1af4, 0x1050, 0, LUID_A);
    let nvidia = adapter_luid("NVIDIA GeForce RTX 3060", 0x10de, 0x2504, 0, LUID_B);
    assert_eq!(
        render_adapter_from_dxgi(Some(0), &[virtio.clone(), warp()]),
        RenderAdapter::Unknown
    );
    assert_eq!(
        render_adapter_from_dxgi(None, &[virtio, nvidia]),
        RenderAdapter::Luid(LUID_B)
    );
}

#[test]
fn amd_intel_and_nvidia_adapters_are_gpus() {
    let got = devices_from_dxgi(&[
        adapter("AMD Radeon RX 7900 XTX", 0x1002, 0x744c, 0),
        adapter("Intel(R) UHD Graphics 770", 0x8086, 0x4680, 0),
        adapter("NVIDIA GeForce RTX 3060", 0x10de, 0x2504, 0),
    ]);
    assert_eq!(
        got,
        vec![
            GpuDevice {
                vendor: Vendor::Amd,
                device_id: Some("744c".into()),
                name: Some("AMD Radeon RX 7900 XTX".into()),
                render_node: true,
            },
            GpuDevice {
                vendor: Vendor::Intel,
                device_id: Some("4680".into()),
                name: Some("Intel(R) UHD Graphics 770".into()),
                render_node: true,
            },
            GpuDevice {
                vendor: Vendor::Nvidia,
                device_id: Some("2504".into()),
                name: Some("NVIDIA GeForce RTX 3060".into()),
                render_node: true,
            },
        ]
    );
}

#[test]
fn software_and_microsoft_adapters_are_not_gpus() {
    let got = devices_from_dxgi(&[
        adapter(
            "Microsoft Basic Render Driver",
            0x1414,
            0x8c,
            DXGI_ADAPTER_FLAG_SOFTWARE,
        ),
        adapter("Microsoft Hyper-V Video", 0x1414, 0x5353, 0),
        adapter(
            "Some software rasterizer",
            0x10de,
            0x1,
            DXGI_ADAPTER_FLAG_SOFTWARE,
        ),
        adapter("Virtio GPU", 0x1af4, 0x1050, 0),
    ]);
    assert!(got.is_empty(), "{got:?}");
}

#[test]
fn amd_only_machine_on_basic_render_counts_its_gpu() {
    // The #942 bug: no nvidia-smi on an AMD-only machine meant no GPUs. DXGI
    // finds the AMD GPU, so this is "a GPU OBS can't use": red (#1005), not
    // the yellow of a machine with no GPU.
    let devices = devices_from_dxgi(&[
        adapter("AMD Radeon RX 6600", 0x1002, 0x73ff, 0),
        adapter(
            "Microsoft Basic Render Driver",
            0x1414,
            0x8c,
            DXGI_ADAPTER_FLAG_SOFTWARE,
        ),
    ]);
    assert_eq!(
        classify(Some("Microsoft Basic Render Driver"), &devices, None),
        State::Red
    );
}

#[test]
fn trailing_spaces_and_nuls_are_trimmed_from_names() {
    let got = devices_from_dxgi(&[adapter("Intel(R) Arc(TM) A770  \0\0", 0x8086, 0x56a0, 0)]);
    assert_eq!(got[0].name.as_deref(), Some("Intel(R) Arc(TM) A770"));
}

#[test]
fn nvidia_smi_does_not_duplicate_a_named_dxgi_gpu() {
    // Review of #958: DXGI names every GPU, and merge_devices only merged
    // into unnamed entries, so nvidia-smi added the RTX 3060 a second time.
    let mut devices = devices_from_dxgi(&[adapter("NVIDIA GeForce RTX 3060", 0x10de, 0x2504, 0)]);
    obs_gpu_status::merge_devices(
        &mut devices,
        obs_gpu_status::parse_nvidia_smi(
            "NVIDIA GeForce RTX 3060, 0x250410DE\nNVIDIA GeForce RTX 2080 Ti, 0x1E0410DE",
        ),
    );
    assert_eq!(devices.len(), 2, "{devices:?}");
    assert_eq!(devices[1].device_id.as_deref(), Some("1e04"));
}
