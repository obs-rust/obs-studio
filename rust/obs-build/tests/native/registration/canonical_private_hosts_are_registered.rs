use super::*;
use obs_build::validation::graph::Graph;

const CASES: [&str; 2] = ["test_graphics_ffmpeg_host", "test_nix_wayland_keys_steps"];

fn expected() -> serde_json::Value {
    serde_json::from_str(include_str!("private_hosts_expected.json")).unwrap()
}

fn graph() -> Graph {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let expected = expected();
    let mut selected = serde_json::json!({"schema": 1, "targets": {}, "tests": {}});
    for section in ["targets", "tests"] {
        for name in CASES {
            assert_eq!(
                raw[section][name], expected[section][name],
                "{section}/{name}"
            );
            selected[section][name] = raw[section][name].clone();
        }
    }
    Graph::parse(&selected.to_string()).unwrap()
}

#[test]
fn canonical_private_hosts_are_registered() {
    let graph = graph();
    for os in ["OS_LINUX", "OS_FREEBSD", "OS_WINDOWS", "OS_MACOS"] {
        for enabled in [false, true] {
            for sdk in [false, true] {
                let mut facts = std::collections::BTreeMap::from([
                    ("ENABLE_WAYLAND".into(), enabled),
                    ("HAVE_NIX_WAYLAND_KEYS_TEST_SDK".into(), sdk),
                ]);
                for host in ["OS_LINUX", "OS_FREEBSD", "OS_WINDOWS", "OS_MACOS"] {
                    facts.insert(host.into(), host == os);
                }
                let selected = graph.select(&facts).unwrap();
                let count = usize::from(os != "OS_WINDOWS")
                    + usize::from(enabled && sdk && matches!(os, "OS_LINUX" | "OS_FREEBSD"));
                assert_eq!(selected.targets.len(), count, "{os}/{enabled}/{sdk}");
                assert_eq!(selected.tests.len(), count, "{os}/{enabled}/{sdk}");
            }
        }
    }
}

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "private_hosts_runtime.rs"]
mod runtime;
