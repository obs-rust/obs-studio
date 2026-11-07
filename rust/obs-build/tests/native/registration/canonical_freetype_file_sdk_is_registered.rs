use super::*;
use obs_build::validation::graph::Graph;

const NAME: &str = "test_text_aa_file";

fn requirement() -> serde_json::Value {
    serde_json::json!({"all": ["OS_LINUX", "HAVE_OBS_TEXT_FREETYPE2", "HAVE_FREETYPE_FILE_SDK"]})
}

fn expected_target() -> serde_json::Value {
    let mut sources = vec![format!("test/cmocka/{NAME}.c")];
    sources.extend(["text-freetype2", "text-functionality", "text-file", "glyph-atlas", "obs-convenience", "find-font-unix", "text-freetype2-host-posix"].map(|name| format!("plugins/text-freetype2/{name}.c.h")));
    serde_json::json!({
        "sources": sources,
        "includes": [".", "plugins/text-freetype2"],
        "definitions": ["TEST_FONT_PATH=\"${source}/test/cmocka/data/text-aa.ttf\""],
        "watch_inputs": ["plugins/text-freetype2/build/text-freetype2-nix.pch.h", "test/cmocka/data/text-aa.ttf"],
        "libraries": ["OBS::libobs", "Freetype::Freetype", "Fontconfig::Fontconfig", "OBS::native-test-cmocka"],
        "requires": requirement()
    })
}

#[test]
fn canonical_freetype_file_sdk_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value = serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap()).unwrap();
    assert_eq!(raw["targets"][NAME], expected_target());
    assert_eq!(raw["tests"][NAME], serde_json::json!({"target": NAME, "requires": requirement()}));
    let fragment = fs::read_to_string(root.join(format!("test/cmocka/tests.d/{NAME}.cmake"))).unwrap();
    assert!(!fragment.contains("add_executable("));
    assert!(!fragment.contains("add_test("));
}

#[test]
fn canonical_freetype_file_sdk_guards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.targets.retain(|name, _| name == NAME);
    graph.tests.retain(|name, _| name == NAME);
    assert_eq!(graph.targets.len(), 1);
    for linux in [false, true] {
        for owner in [false, true] {
            for sdk in [false, true] {
                let facts = [("OS_LINUX".into(), linux), ("HAVE_OBS_TEXT_FREETYPE2".into(), owner), ("HAVE_FREETYPE_FILE_SDK".into(), sdk)].into();
                let selected = graph.select(&facts).unwrap();
                let count = usize::from(linux && owner && sdk);
                assert_eq!(selected.targets.len(), count);
                assert_eq!(selected.tests.len(), count);
            }
        }
    }
}
