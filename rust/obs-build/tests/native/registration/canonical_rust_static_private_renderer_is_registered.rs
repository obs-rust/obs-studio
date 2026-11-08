use super::*;

const FAMILY: &str = "private-renderer";

#[test]
fn canonical_rust_static_private_renderer_is_registered() {
    let graph = rust_static_entries::assert_family(FAMILY);
    for platform in ["OS_LINUX", "OS_WINDOWS", "OS_MACOS"] {
        for archive in [false, true] {
            for renderer in [false, true] {
                let facts = [
                    ("OS_LINUX".into(), platform == "OS_LINUX"),
                    ("OS_WINDOWS".into(), platform == "OS_WINDOWS"),
                    ("OS_MACOS".into(), platform == "OS_MACOS"),
                    ("HAVE_LIBOBS_RUST_STATIC".into(), archive),
                    ("HAVE_OBS_OPENGL".into(), renderer),
                ]
                .into();
                let selected = graph.select(&facts).unwrap();
                let expected = usize::from(platform == "OS_LINUX" && archive && renderer);
                assert_eq!(selected.targets.len(), expected);
                assert_eq!(selected.tests.len(), expected);
            }
        }
    }
}

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
mod runtime {
    #[test]
    fn canonical_private_renderer_runs_unchanged() {
        super::super::graphics_private_runtime::run(
            super::FAMILY,
            &super::super::rust_static_entries::cases(super::FAMILY),
        );
    }
}

#[test]
fn canonical_rust_static_private_parsers_are_registered() {
    let graph = rust_static_entries::assert_family("private-parser");
    for windows in [false, true] {
        for msvc in [false, true] {
            for archive in [false, true] {
                let facts = [
                    ("OS_WINDOWS".into(), windows),
                    ("MSVC".into(), msvc),
                    ("HAVE_LIBOBS_RUST_STATIC".into(), archive),
                ]
                .into();
                let selected = graph.select(&facts).unwrap();
                assert_eq!(selected.targets.len(), if archive { 4 } else { 0 });
                assert_eq!(selected.tests.len(), if archive { 4 } else { 0 });
            }
        }
    }
}

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[test]
fn canonical_private_parsers_run_unchanged() {
    graphics_private_runtime::run(
        "private-parser",
        &rust_static_entries::cases("private-parser"),
    );
}
