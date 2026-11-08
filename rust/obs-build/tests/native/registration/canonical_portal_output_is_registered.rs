use super::*;
use obs_build::validation::graph::Graph;

fn expected() -> Graph {
    Graph::parse(include_str!("portal_output_expected.json")).unwrap()
}

#[test]
fn canonical_portal_output_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let actual = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let expected = expected();
    for (name, target) in &expected.targets {
        assert_eq!(format!("{:?}", actual.targets.get(name).expect(name)), format!("{target:?}"));
        assert_eq!(format!("{:?}", actual.tests.get(name).unwrap()), format!("{:?}", expected.tests[name]));
    }
    for host in ["OS_LINUX", "OS_FREEBSD", "OS_WINDOWS", "OS_MACOS"] {
        for gio in [false, true] {
            for renderer in [false, true] {
                let mut facts = std::collections::BTreeMap::from([
                    ("UNIX".into(), host != "OS_WINDOWS"),
                    ("HAVE_PORTAL_TEST_GIO".into(), gio),
                    ("HAVE_OBS_OPENGL".into(), renderer),
                ]);
                for os in ["OS_LINUX", "OS_FREEBSD", "OS_WINDOWS", "OS_MACOS"] {
                    facts.insert(os.into(), os == host);
                }
                let selected = expected.select(&facts).unwrap();
                let count = 1 + usize::from(host != "OS_WINDOWS")
                    + 3 * usize::from(gio && matches!(host, "OS_LINUX" | "OS_FREEBSD"))
                    + usize::from(renderer && host == "OS_LINUX");
                assert_eq!(selected.targets.len(), count);
                assert_eq!(selected.tests.len(), count);
            }
        }
    }
}

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "portal_output_runtime.rs"]
mod runtime;
