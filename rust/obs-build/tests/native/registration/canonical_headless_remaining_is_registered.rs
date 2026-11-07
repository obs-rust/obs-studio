use super::*;
use obs_build::validation::graph::Graph;
const CASES: [&str; 6] = ["obs-x264-test", "test_first_run_wizard", "test_config_migration", "test_core_module_manifest", "test_preflight_abi", "test_scripting_obs_defs"];
fn expected() -> serde_json::Value {
    serde_json::from_str(include_str!("headless_remaining_expected.json")).unwrap()
}
#[test]
fn canonical_headless_remaining_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value = serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap()).unwrap();
    for section in ["targets", "tests"] {
        for (name, value) in expected()[section].as_object().unwrap() {
            assert_eq!(&raw[section][name], value, "{section}/{name}");
        }
    }
}
#[test]
fn canonical_headless_remaining_guards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.targets.retain(|n, _| CASES.contains(&n.as_str()));
    graph.tests.retain(|_, t| CASES.contains(&t.target.as_str()));
    assert_eq!(graph.targets.len(), 6);
    for os in ["OS_LINUX", "OS_FREEBSD", "OS_OPENBSD", "OS_WINDOWS", "OS_MACOS"] {
        for enabled in [false, true] {
            let mut facts: std::collections::BTreeMap<String, bool> = ["OS_LINUX", "OS_FREEBSD", "OS_OPENBSD", "OS_WINDOWS", "OS_MACOS"].into_iter().map(|n| (n.into(), n == os)).collect();
            for n in ["HAVE_FRONTEND_PLAIN_TEST_OWNER", "HAVE_X264_TEST_OWNER", "ENABLE_CORE_MODULES", "HAVE_PREFLIGHT_TEST_ARCHIVE", "HAVE_SCRIPTING_DEFS_TEST_SDK"] { facts.insert(n.into(), enabled); }
            facts.insert("C_COMPILER_CLANG".into(), false);
            let selected = graph.select(&facts).unwrap();
            let count = if enabled { 4 + usize::from(["OS_LINUX", "OS_MACOS"].contains(&os)) + usize::from(["OS_LINUX", "OS_FREEBSD", "OS_OPENBSD"].contains(&os)) } else { 0 };
            assert_eq!(selected.targets.len(), count, "{os}/{enabled}");
            assert_eq!(selected.tests.len(), if enabled {count + 6} else {0});
        }
    }
}
