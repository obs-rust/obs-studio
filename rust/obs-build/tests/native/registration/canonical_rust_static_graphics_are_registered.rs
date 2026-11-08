use super::*;

static CASES: std::sync::LazyLock<Vec<String>> =
    std::sync::LazyLock::new(|| rust_static_entries::cases("graphics"));

#[test]
fn canonical_rust_static_graphics_are_registered() {
    let graph = rust_static_entries::assert_family("graphics");
    assert_eq!(CASES.len(), 6);
    for windows in [false, true] {
        for msvc in [false, true] {
            for available in [false, true] {
                let facts = [
                    ("HAVE_LIBOBS_RUST_STATIC".into(), available),
                    ("OS_WINDOWS".into(), windows),
                    ("MSVC".into(), msvc),
                ]
                .into();
                let selected = graph.select(&facts).unwrap();
                assert_eq!(selected.targets.len(), if available { 6 } else { 0 });
                assert_eq!(selected.tests.len(), if available { 6 } else { 0 });
            }
        }
    }
}

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "graphics_private_runtime.rs"]
mod runtime;
