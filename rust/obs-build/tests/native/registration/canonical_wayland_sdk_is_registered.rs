use super::*;
use obs_build::validation::graph::Graph;

const CASES: [&str; 3] = ["test_wayland_keymap", "test_wayland_volume_keys", "test_wayland_multiseat"];
const EXPECTED: &str = r#"{
  "targets": {
    "test_wayland_keymap": {
      "sources": [
        "test/cmocka/test_wayland_keymap.c",
        "libobs/obs-nix-wayland-key-names.c.h",
        "libobs/obs-nix-wayland-keys.c.h",
        "libobs/obs-nix-wayland-seats.c.h",
        "libobs/obs-nix-hotkey-state.c.h",
        "test/cmocka/hotkey_mutex_stub.c"
      ],
      "includes": [
        ".",
        "libobs"
      ],
      "definitions": [
        "obs=wayland_test_obs"
      ],
      "watch_inputs": [
        "libobs/build/libobs-wayland.pch.h"
      ],
      "libraries": [
        "OBS::libobs",
        "OBS::caption",
        "Wayland::Client",
        "xkbcommon::xkbcommon",
        "OBS::native-test-cmocka"
      ],
      "requires": {
        "all": [
          {
            "any": [
              "OS_LINUX",
              "OS_FREEBSD"
            ]
          },
          "ENABLE_WAYLAND",
          "HAVE_WAYLAND_CLIENT_TEST_SDK"
        ]
      }
    },
    "test_wayland_volume_keys": {
      "sources": [
        "test/cmocka/test_wayland_volume_keys.c",
        "libobs/obs-nix-wayland.c.h",
        "libobs/obs-nix-wayland-key-names.c.h",
        "libobs/obs-nix-wayland-keys.c.h",
        "libobs/obs-nix-wayland-seats.c.h",
        "libobs/obs-nix-hotkey-state.c.h",
        "test/cmocka/hotkey_mutex_stub.c"
      ],
      "includes": [
        ".",
        "libobs"
      ],
      "definitions": [
        "obs=wayland_test_obs"
      ],
      "watch_inputs": [
        "libobs/build/libobs-wayland.pch.h"
      ],
      "libraries": [
        "OBS::libobs",
        "OBS::caption",
        "Wayland::Client",
        "xkbcommon::xkbcommon",
        "OBS::native-test-cmocka"
      ],
      "requires": {
        "all": [
          {
            "any": [
              "OS_LINUX",
              "OS_FREEBSD"
            ]
          },
          "ENABLE_WAYLAND",
          "HAVE_WAYLAND_CLIENT_TEST_SDK"
        ]
      }
    },
    "test_wayland_multiseat": {
      "sources": [
        "test/cmocka/test_wayland_multiseat.c",
        "libobs/obs-nix-wayland.c.h",
        "libobs/obs-nix-wayland-key-names.c.h",
        "libobs/obs-nix-wayland-keys.c.h",
        "libobs/obs-nix-wayland-seats.c.h",
        "libobs/obs-nix-hotkey-state.c.h",
        "test/cmocka/hotkey_mutex_stub.c"
      ],
      "includes": [
        ".",
        "libobs"
      ],
      "definitions": [
        "obs=wayland_test_obs"
      ],
      "watch_inputs": [
        "libobs/build/libobs-wayland.pch.h"
      ],
      "libraries": [
        "OBS::libobs",
        "OBS::caption",
        "Wayland::Client",
        "Wayland::Server",
        "xkbcommon::xkbcommon",
        "OBS::native-test-cmocka"
      ],
      "requires": {
        "all": [
          {
            "any": [
              "OS_LINUX",
              "OS_FREEBSD"
            ]
          },
          "ENABLE_WAYLAND",
          "HAVE_WAYLAND_SERVER_TEST_SDK"
        ]
      }
    }
  },
  "tests": {
    "test_wayland_keymap": {
      "target": "test_wayland_keymap",
      "requires": {
        "all": [
          {
            "any": [
              "OS_LINUX",
              "OS_FREEBSD"
            ]
          },
          "ENABLE_WAYLAND",
          "HAVE_WAYLAND_CLIENT_TEST_SDK"
        ]
      }
    },
    "test_wayland_volume_keys": {
      "target": "test_wayland_volume_keys",
      "requires": {
        "all": [
          {
            "any": [
              "OS_LINUX",
              "OS_FREEBSD"
            ]
          },
          "ENABLE_WAYLAND",
          "HAVE_WAYLAND_CLIENT_TEST_SDK"
        ]
      }
    },
    "test_wayland_multiseat": {
      "target": "test_wayland_multiseat",
      "requires": {
        "all": [
          {
            "any": [
              "OS_LINUX",
              "OS_FREEBSD"
            ]
          },
          "ENABLE_WAYLAND",
          "HAVE_WAYLAND_SERVER_TEST_SDK"
        ]
      }
    }
  }
}"#;

#[test]
fn canonical_wayland_sdk_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value = serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap()).unwrap();
    let expected: serde_json::Value = serde_json::from_str(EXPECTED).unwrap();
    for name in CASES {
        for section in ["targets", "tests"] {
            assert_eq!(raw[section][name], expected[section][name], "{section}: {name}");
        }
        let fragment = fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
    }
}

#[test]
fn canonical_wayland_sdk_guards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.targets.retain(|name, _| CASES.contains(&name.as_str()));
    graph.tests.retain(|name, _| CASES.contains(&name.as_str()));
    assert_eq!(graph.targets.len(), CASES.len());
    for os in ["OS_LINUX", "OS_FREEBSD", "OS_OPENBSD", "OS_WINDOWS", "OS_MACOS"] {
        for enabled in [false, true] {
            for client in [false, true] {
                for server in [false, true] {
                    assert_selection(&graph, os, enabled, client, server);
                }
            }
        }
    }
}

fn assert_selection(graph: &Graph, os: &str, enabled: bool, client: bool, server: bool) {
    let facts = [
        ("OS_LINUX".into(), os == "OS_LINUX"),
        ("OS_FREEBSD".into(), os == "OS_FREEBSD"),
        ("ENABLE_WAYLAND".into(), enabled),
        ("HAVE_WAYLAND_CLIENT_TEST_SDK".into(), client),
        ("HAVE_WAYLAND_SERVER_TEST_SDK".into(), server),
    ].into();
    let selection = graph.select(&facts).unwrap();
    let count = if enabled && ["OS_LINUX", "OS_FREEBSD"].contains(&os) { 2 * usize::from(client) + usize::from(server) } else { 0 };
    assert_eq!(selection.targets.len(), count);
    assert_eq!(selection.tests.len(), count);
}
