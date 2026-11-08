use super::*;
use obs_build::validation::graph::{ConditionalValue, Dependency};
use std::path::PathBuf;

const FAMILY: &str = "windows-keys";

#[test]
fn canonical_rust_static_windows_keys_are_registered() {
    let graph = rust_static_entries::assert_family(FAMILY);
    let name = "test_windows_keys_steps_rust";
    for host in ["OS_LINUX", "OS_FREEBSD", "OS_WINDOWS", "OS_MACOS"] {
        for archive in [false, true] {
            for uuid in [false, true] {
                for gio in [false, true] {
                    let mut facts = std::collections::BTreeMap::from([
                        ("HAVE_LIBOBS_RUST_STATIC".into(), archive),
                        ("HAVE_WINDOWS_KEYS_TEST_UUID".into(), uuid),
                        ("HAVE_WINDOWS_KEYS_TEST_GIO".into(), gio),
                    ]);
                    for os in ["OS_LINUX", "OS_FREEBSD", "OS_WINDOWS", "OS_MACOS"] {
                        facts.insert(os.into(), os == host);
                    }
                    let selected = graph.select(&facts).unwrap();
                    assert_eq!(selected.targets.len(), usize::from(archive));
                    assert_eq!(selected.tests.len(), usize::from(archive));
                    if !archive {
                        continue;
                    }
                    let target = selected.targets[name];
                    let interfaces = target.libraries.iter().map(|library| {
                        let (ConditionalValue::Name(name) | ConditionalValue::Conditional { name, .. }) = library;
                        (name.clone(), Dependency { artifacts: vec![PathBuf::from(name)], ..Default::default() })
                    }).collect();
                    let dependencies = target.dependency_artifacts(&facts, &interfaces).unwrap();
                    let contains = |name| dependencies.contains(&PathBuf::from(name));
                    // Public references originate in the Rust archive, so the core must
                    // follow it even with GNU ld's strict --as-needed policy.
                    assert_eq!(&dependencies[..2], &[PathBuf::from("libobs_rust-static"), PathBuf::from("OBS::libobs")]);
                    assert_eq!(contains("LibUUID::LibUUID"), uuid);
                    for library in ["gio::gio", "gobject-2.0", "glib-2.0"] {
                        assert_eq!(contains(library), gio);
                    }
                    for library in ["OBS::w32-pthreads", "winmm", "Rpcrt4"] {
                        assert_eq!(contains(library), host == "OS_WINDOWS");
                    }
                    assert_eq!(target.conditional_link_options.len(), 4);
                }
            }
        }
    }
}

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[test]
fn canonical_windows_keys_run_unchanged() {
    graphics_private_runtime::run(FAMILY, &rust_static_entries::cases(FAMILY));
}
