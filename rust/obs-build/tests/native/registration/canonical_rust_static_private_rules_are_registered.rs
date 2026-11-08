use super::*;

const FAMILY: &str = "private-rules";

static CASES: std::sync::LazyLock<Vec<String>> =
    std::sync::LazyLock::new(|| rust_static_entries::cases(FAMILY));

#[test]
fn canonical_rust_static_private_rules_are_registered() {
    let graph = rust_static_entries::assert_family(FAMILY);
    assert_eq!(CASES.len(), 2);
    for os in ["OS_LINUX", "OS_FREEBSD", "OS_WINDOWS", "OS_MACOS"] {
        for msvc in [false, true] {
            for available in [false, true] {
                for pulse in [false, true] {
                    let mut facts = [
                        ("HAVE_LIBOBS_RUST_STATIC".into(), available),
                        ("MSVC".into(), msvc),
                        ("ENABLE_PULSEAUDIO".into(), pulse),
                    ]
                    .into_iter()
                    .collect::<std::collections::BTreeMap<_, _>>();
                    for host in ["OS_LINUX", "OS_FREEBSD", "OS_WINDOWS", "OS_MACOS"] {
                        facts.insert(host.into(), host == os);
                    }
                    let selected = graph.select(&facts).unwrap();
                    let expected = if available {
                        1 + usize::from(pulse && matches!(os, "OS_LINUX" | "OS_FREEBSD"))
                    } else {
                        0
                    };
                    assert_eq!(selected.targets.len(), expected);
                    assert_eq!(selected.tests.len(), expected);
                }
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
