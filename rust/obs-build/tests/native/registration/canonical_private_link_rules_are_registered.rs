use super::*;
use std::path::PathBuf;
use obs_build::validation::graph::Graph;

const CASES: [&str; 2] = ["test_graphics_state_stacks_rust", "test_window_match_steps_rust"];

fn expected() -> serde_json::Value {
    serde_json::from_str(include_str!("private_link_rules_expected.json")).unwrap()
}

fn graph() -> Graph {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap(),
    ).unwrap();
    let expected = expected();
    let mut selected = serde_json::json!({"schema": 1, "targets": {}, "tests": {}});
    for section in ["targets", "tests"] {
        for name in CASES {
            assert_eq!(raw[section][name], expected[section][name], "{section}/{name}");
            selected[section][name] = raw[section][name].clone();
        }
    }
    Graph::parse(&selected.to_string()).unwrap()
}

#[test]
fn canonical_private_link_rules_are_registered() {
    let graph = graph();
    for os in ["OS_LINUX", "OS_FREEBSD", "OS_WINDOWS", "OS_MACOS"] {
        for archive in [false, true] {
            for gio in [false, true] {
                let mut facts = std::collections::BTreeMap::from([
                    ("HAVE_LIBOBS_RUST_STATIC".into(), archive),
                    ("HAVE_WINDOW_MATCH_TEST_GIO".into(), gio),
                    ("MSVC".into(), os == "OS_WINDOWS"),
                ]);
                for host in ["OS_LINUX", "OS_FREEBSD", "OS_WINDOWS", "OS_MACOS"] {
                    facts.insert(host.into(), host == os);
                }
                let selected = graph.select(&facts).unwrap();
                let count = usize::from(archive) * (1 + usize::from(os == "OS_LINUX"));
                assert_eq!(selected.targets.len(), count, "{os}/{archive}/{gio}");
                assert_eq!(selected.tests.len(), count, "{os}/{archive}/{gio}");
                let mut interfaces = std::collections::BTreeMap::new();
                for name in ["OBS::libobs", "OBS::caption", "OBS::native-test-cmocka", "libobs_rust-static", "m", "OBS::w32-pthreads", "LibUUID::LibUUID", "PkgConfig::_window_match_test_gio"] {
                    interfaces.insert(name.into(), obs_build::validation::graph::Dependency {
                        artifacts: vec![PathBuf::from(name)], ..Default::default()
                    });
                }
                let state = &graph.targets[CASES[0]];
                let artifacts = state.dependency_artifacts(&facts, &interfaces).unwrap();
                assert_eq!(artifacts.iter().any(|p| p == Path::new("m")), os != "OS_WINDOWS");
                assert_eq!(artifacts.iter().any(|p| p == Path::new("OBS::w32-pthreads")), os == "OS_WINDOWS");
                let window = &graph.targets[CASES[1]];
                let artifacts = window.dependency_artifacts(&facts, &interfaces).unwrap();
                assert_eq!(artifacts.last().unwrap(), Path::new(if gio {"PkgConfig::_window_match_test_gio"} else {"LibUUID::LibUUID"}));
            }
        }
    }
}
