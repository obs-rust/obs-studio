//! Tier 1 tests for obs-rust/obs-studio#1020: which PCI device OBS renders
//! on. New code with no C original, so Tiers 2 and 3 do not apply.

use std::fs;
#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;

use obs_gpu_status::{RenderAdapter, render_adapter, render_adapter_from_registry};

struct Tree(PathBuf);

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn tree(name: &str) -> Tree {
    let p = std::env::temp_dir().join(format!(
        "obs-gpu-render-adapter-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    Tree(p)
}

/// A GPU at `pci` with a card and a render node, laid out as sysfs does:
/// `/sys/class/drm/<node>` links into the PCI device, and the node's
/// `device` link points back up to `../../../<pci>`.
#[cfg(unix)]
fn gpu(root: &Path, pci: &str, card: &str, node: &str, vendor: &str, device: &str) {
    let dev = root.join("sys/devices/pci0000:00").join(pci);
    fs::create_dir_all(&dev).unwrap();
    fs::write(dev.join("vendor"), format!("{vendor}\n")).unwrap();
    fs::write(dev.join("device"), format!("{device}\n")).unwrap();
    let class = root.join("sys/class/drm");
    fs::create_dir_all(&class).unwrap();
    for name in [card, node] {
        let dir = dev.join("drm").join(name);
        fs::create_dir_all(&dir).unwrap();
        symlink(&format!("../../../{pci}"), &dir.join("device"));
        symlink(
            &format!("../../devices/pci0000:00/{pci}/drm/{name}"),
            &class.join(name),
        );
    }
}

#[cfg(unix)]
fn symlink(target: &str, link: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[cfg(unix)]
fn two_identical_cards(name: &str) -> Tree {
    let t = tree(name);
    gpu(
        &t.0,
        "0000:2b:00.0",
        "card0",
        "renderD128",
        "0x10de",
        "0x2504",
    );
    gpu(
        &t.0,
        "0000:2c:00.0",
        "card1",
        "renderD129",
        "0x10de",
        "0x2504",
    );
    t
}

#[cfg(unix)]
#[test]
fn render_node_picks_its_own_card_among_identical_cards() {
    let t = two_identical_cards("identical");
    assert_eq!(
        render_adapter(&t.0, Some("/dev/dri/renderD129")),
        RenderAdapter::Pci("0000:2c:00.0".into())
    );
    assert_eq!(
        render_adapter(&t.0, Some("/dev/dri/renderD128")),
        RenderAdapter::Pci("0000:2b:00.0".into())
    );
}

#[cfg(unix)]
#[test]
fn no_render_node_with_two_cards_is_ambiguous() {
    // GLX, or EGL without EGL_EXT_device_drm_render_node: never guess.
    let t = two_identical_cards("ambiguous");
    assert_eq!(render_adapter(&t.0, None), RenderAdapter::Ambiguous);
}

#[cfg(unix)]
#[test]
fn no_render_node_with_one_card_is_that_card() {
    let t = tree("single");
    gpu(
        &t.0,
        "0000:2b:00.0",
        "card1",
        "renderD128",
        "0x10de",
        "0x2504",
    );
    assert_eq!(
        render_adapter(&t.0, None),
        RenderAdapter::Pci("0000:2b:00.0".into())
    );
}

#[cfg(unix)]
#[test]
fn software_rendering_is_unknown() {
    // gl_egl_enum_adapters names the adapter "/Software" without a render node.
    let t = two_identical_cards("software");
    assert_eq!(
        render_adapter(&t.0, Some("/Software")),
        RenderAdapter::Unknown
    );
}

#[cfg(unix)]
#[test]
fn unknown_render_node_is_unknown() {
    let t = two_identical_cards("missing");
    assert_eq!(
        render_adapter(&t.0, Some("/dev/dri/renderD200")),
        RenderAdapter::Unknown
    );
    assert_eq!(
        render_adapter(&t.0, Some("/dev/dri/../../etc/passwd")),
        RenderAdapter::Unknown
    );
}

#[cfg(unix)]
#[test]
fn non_pci_device_is_unknown() {
    // A platform GPU (Arm SoC): the device link does not end in a PCI address.
    let t = tree("platform");
    let dev = t.0.join("sys/devices/platform/fde60000.gpu");
    fs::create_dir_all(dev.join("drm/renderD128")).unwrap();
    symlink("../../../fde60000.gpu", &dev.join("drm/renderD128/device"));
    fs::create_dir_all(t.0.join("sys/class/drm")).unwrap();
    symlink(
        "../../devices/platform/fde60000.gpu/drm/renderD128",
        &t.0.join("sys/class/drm/renderD128"),
    );
    assert_eq!(
        render_adapter(&t.0, Some("/dev/dri/renderD128")),
        RenderAdapter::Unknown
    );
}

#[test]
fn missing_sysfs_is_unknown() {
    let t = tree("empty");
    assert_eq!(
        render_adapter(&t.0, Some("/dev/dri/renderD128")),
        RenderAdapter::Unknown
    );
    assert_eq!(render_adapter(&t.0, None), RenderAdapter::Unknown);
}

// macOS (obs-rust/obs-studio#1020): registry IDs. 0x100000b5b is the M1 Max
// (`MTLDevice.registryID` and its AGXAccelerator entry); the pair below is a
// MacBook Pro with an integrated and a discrete GPU.
const APPLE_SILICON: u64 = 0x1_0000_0b5b;
const INTEGRATED: u64 = 0x1_0000_0401;
const DISCRETE: u64 = 0x1_0000_0502;

#[test]
fn metal_default_gpu_is_the_render_adapter() {
    assert_eq!(
        render_adapter_from_registry(Some(APPLE_SILICON), &[APPLE_SILICON]),
        RenderAdapter::RegistryId(APPLE_SILICON)
    );
    assert_eq!(
        render_adapter_from_registry(Some(DISCRETE), &[INTEGRATED, DISCRETE]),
        RenderAdapter::RegistryId(DISCRETE)
    );
}

#[test]
fn metal_default_without_iokit_list_is_still_used() {
    // IOKit enumeration failed, but Metal named its device.
    assert_eq!(
        render_adapter_from_registry(Some(APPLE_SILICON), &[]),
        RenderAdapter::RegistryId(APPLE_SILICON)
    );
}

#[test]
fn metal_default_missing_from_iokit_is_unknown() {
    // The two sources disagree: never guess.
    assert_eq!(
        render_adapter_from_registry(Some(DISCRETE), &[INTEGRATED]),
        RenderAdapter::Unknown
    );
}

#[test]
fn no_metal_device_with_one_gpu_is_that_gpu() {
    assert_eq!(
        render_adapter_from_registry(None, &[APPLE_SILICON, APPLE_SILICON]),
        RenderAdapter::RegistryId(APPLE_SILICON)
    );
}

#[test]
fn no_metal_device_with_two_gpus_is_ambiguous() {
    assert_eq!(
        render_adapter_from_registry(None, &[INTEGRATED, DISCRETE]),
        RenderAdapter::Ambiguous
    );
}

#[test]
fn no_metal_device_and_no_gpu_is_unknown() {
    assert_eq!(
        render_adapter_from_registry(None, &[]),
        RenderAdapter::Unknown
    );
}

#[test]
fn host_render_adapter_does_not_panic() {
    // The real platform path: sysfs on Linux, Metal and IOKit on macOS,
    // DXGI on Windows (CI's Windows runner has only Microsoft adapters).
    let _ = obs_gpu_status::platform::render_adapter(obs_gpu_status::RenderHint::default());
    let _ = obs_gpu_status::platform::render_adapter(obs_gpu_status::RenderHint {
        adapter_index: Some(0),
        render_node: Some("/dev/dri/renderD128"),
    });
}
