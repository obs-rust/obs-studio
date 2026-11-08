use super::*;

const FAMILY: &str = "private-steps";

static CASES: std::sync::LazyLock<Vec<String>> =
    std::sync::LazyLock::new(|| rust_static_entries::cases(FAMILY));

#[test]
fn canonical_rust_static_private_steps_are_registered() {
    let graph = rust_static_entries::assert_family(FAMILY);
    assert_eq!(CASES.len(), 5);
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
                assert_eq!(selected.targets.len(), if available { 5 } else { 0 });
                assert_eq!(selected.tests.len(), if available { 5 } else { 0 });
            }
        }
    }
}

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
mod runtime {
    #[test]
    fn canonical_graphics_private_runs_unchanged() {
        super::super::graphics_private_runtime::run(super::FAMILY, &super::CASES);
    }
}
