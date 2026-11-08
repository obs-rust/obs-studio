use super::*;
use obs_build::validation::graph::Graph;
const CASES: [&str; 6] = [
    "test_inhibit_sleep",
    "test_file_updater_failure",
    "test_file_updater_timeout",
    "test_frontend_warn_fallback",
    "test_text_freetype2_face_index",
    "test_text_freetype2_bundled_font",
];
#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "headless_services_runtime.rs"]
mod runtime;
fn expected_test_inhibit_sleep() -> serde_json::Value {
    serde_json::json!({
        "sources": [
            "test/cmocka/test_inhibit_sleep.c"
        ],
        "libraries": [
            "OBS::libobs",
            "OBS::native-test-cmocka"
        ],
        "requires": {
            "any": [
                "OS_LINUX",
                "OS_FREEBSD"
            ]
        }
    })
}

fn expected_test_file_updater_failure() -> serde_json::Value {
    serde_json::json!({
        "sources": [
            "test/cmocka/test_file_updater_failure.c",
            "shared/file-updater/build/file-updater.core+.h.c"
        ],
        "libraries": [
            "OBS::libobs",
            "CURL::libcurl",
            "OBS::native-test-cmocka"
        ],
        "requires": "OS_LINUX",
        "includes": [
            "shared/file-updater"
        ],
        "watch_inputs": [
            "shared/file-updater/build/file-updater.pch.h",
            "shared/file-updater/file-updater/file-updater.c.h",
            "shared/file-updater/file-updater/file-updater.h",
            "shared/file-updater/file-updater/curl-accept-encoding.h"
        ]
    })
}

fn expected_test_file_updater_timeout() -> serde_json::Value {
    serde_json::json!({
        "sources": [
            "test/cmocka/test_file_updater_timeout.c",
            "shared/file-updater/build/file-updater.core+.h.c"
        ],
        "libraries": [
            "OBS::libobs",
            "CURL::libcurl",
            "OBS::native-test-cmocka"
        ],
        "requires": "OS_LINUX",
        "includes": [
            "shared/file-updater"
        ],
        "watch_inputs": [
            "shared/file-updater/build/file-updater.pch.h",
            "shared/file-updater/file-updater/file-updater.c.h",
            "shared/file-updater/file-updater/file-updater.h",
            "shared/file-updater/file-updater/curl-accept-encoding.h"
        ]
    })
}

fn expected_test_frontend_warn_fallback() -> serde_json::Value {
    serde_json::json!({
        "sources": [
            "test/cmocka/test_frontend_warn_fallback.c"
        ],
        "libraries": [
            "OBS::libobs",
            "OBS::frontend-api",
            "OBS::native-test-cmocka"
        ],
        "requires": {
            "all": [
                "OS_LINUX",
                "HAVE_FRONTEND_WARN_TEST_OWNER"
            ]
        },
        "watch_inputs": [
            "frontend/api/build/obs-frontend-api.core+.hpp.cpp",
            "frontend/api/build/obs-frontend-api.pch.hpp",
            "frontend/api/obs-frontend-api.cpp.hpp",
            "frontend/api/obs-frontend-internal.hpp"
        ]
    })
}

fn expected_test_text_freetype2_face_index() -> serde_json::Value {
    serde_json::json!({
        "sources": [
            "test/cmocka/test_text_freetype2_face_index.c",
            "plugins/text-freetype2/find-font-unix.c.h",
            "plugins/text-freetype2/text-freetype2-host-posix.c.h"
        ],
        "libraries": [
            "OBS::libobs",
            "Freetype::Freetype",
            "Fontconfig::Fontconfig",
            "OBS::native-test-cmocka"
        ],
        "requires": {
            "all": [
                "OS_LINUX",
                "HAVE_FONTCONFIG_TEST_OWNER"
            ]
        },
        "includes": [
            "plugins/text-freetype2"
        ],
        "watch_inputs": [
            "plugins/text-freetype2/build/text-freetype2-nix.pch.h",
            "plugins/text-freetype2/find-font.h",
            "plugins/text-freetype2/text-freetype2.h",
            "plugins/text-freetype2/text-freetype2-host.h",
            "test/cmocka/data/text-collection.ttc"
        ],
        "definitions": [
            "COLLECTION_FONTCONFIG=\"${build}/ft2-collection-fonts.conf\""
        ]
    })
}

fn expected_test_text_freetype2_bundled_font() -> serde_json::Value {
    serde_json::json!({
        "sources": [
            "test/cmocka/test_text_freetype2_bundled_font.c",
            "plugins/text-freetype2/find-font-unix.c.h",
            "plugins/text-freetype2/text-freetype2-host-posix.c.h"
        ],
        "libraries": [
            "OBS::libobs",
            "Freetype::Freetype",
            "Fontconfig::Fontconfig",
            "OBS::native-test-cmocka"
        ],
        "requires": {
            "all": [
                "OS_LINUX",
                "HAVE_FONTCONFIG_TEST_OWNER"
            ]
        },
        "includes": [
            "plugins/text-freetype2"
        ],
        "watch_inputs": [
            "plugins/text-freetype2/build/text-freetype2-nix.pch.h",
            "plugins/text-freetype2/find-font.h",
            "plugins/text-freetype2/text-freetype2.h",
            "plugins/text-freetype2/text-freetype2-host.h",
            "plugins/text-freetype2/data/fonts"
        ],
        "definitions": [
            "BUNDLED_FONT_DIR=\"${source}/plugins/text-freetype2/data/fonts\"",
            "ISOLATED_FONTCONFIG=\"${build}/ft2-isolated-fonts.conf\""
        ]
    })
}
fn expected() -> serde_json::Value {
    serde_json::json!({"schema": 1, "targets": {
                "test_inhibit_sleep": expected_test_inhibit_sleep(),
                "test_file_updater_failure": expected_test_file_updater_failure(),
                "test_file_updater_timeout": expected_test_file_updater_timeout(),
                "test_frontend_warn_fallback": expected_test_frontend_warn_fallback(),
                "test_text_freetype2_face_index": expected_test_text_freetype2_face_index(),
                "test_text_freetype2_bundled_font": expected_test_text_freetype2_bundled_font()
            }, "tests": {
        "test_inhibit_sleep": {
            "target": "test_inhibit_sleep",
            "requires": {
                "any": [
                    "OS_LINUX",
                    "OS_FREEBSD"
                ]
            },
            "command": [
                "${target}"
            ],
            "timeout_seconds": 60
        },
        "test_file_updater_failure": {
            "target": "test_file_updater_failure",
            "requires": "OS_LINUX",
            "command": [
                "${target}"
            ],
            "timeout_seconds": 60
        },
        "test_file_updater_timeout": {
            "target": "test_file_updater_timeout",
            "requires": "OS_LINUX",
            "command": [
                "${target}"
            ],
            "timeout_seconds": 200
        },
        "test_frontend_warn_fallback": {
            "target": "test_frontend_warn_fallback",
            "requires": {
                "all": [
                    "OS_LINUX",
                    "HAVE_FRONTEND_WARN_TEST_OWNER"
                ]
            },
            "command": [
                "${target}"
            ]
        },
        "test_text_freetype2_face_index": {
            "target": "test_text_freetype2_face_index",
            "requires": {
                "all": [
                    "OS_LINUX",
                    "HAVE_FONTCONFIG_TEST_OWNER"
                ]
            },
            "command": [
                "${target}"
            ]
        },
        "test_text_freetype2_bundled_font": {
            "target": "test_text_freetype2_bundled_font",
            "requires": {
                "all": [
                    "OS_LINUX",
                    "HAVE_FONTCONFIG_TEST_OWNER"
                ]
            },
            "command": [
                "${target}"
            ]
        },
        "test_text_freetype2_bundled_font_host_font": {
            "target": "test_text_freetype2_bundled_font",
            "requires": {
                "all": [
                    "OS_LINUX",
                    "HAVE_FONTCONFIG_TEST_OWNER"
                ]
            },
            "command": [
                "${target}"
            ],
            "environment": {
                "FONTCONFIG_FILE": "${build}/ft2-host-fonts.conf"
            }
        }
    }})
}
#[test]
fn canonical_headless_services_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let expected = expected();
    for section in ["targets", "tests"] {
        for (name, value) in expected[section].as_object().unwrap() {
            assert_eq!(&raw[section][name], value, "{section}/{name}");
        }
    }
    for name in CASES {
        let text =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(text.contains("test-graph.json"));
        assert!(!text.contains("add_executable("));
        assert!(!text.contains("add_test("));
    }
}
#[test]
fn canonical_headless_services_guards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.targets.retain(|n, _| CASES.contains(&n.as_str()));
    graph
        .tests
        .retain(|_, t| CASES.contains(&t.target.as_str()));
    assert_eq!(graph.targets.len(), 6);
    for os in [
        "OS_LINUX",
        "OS_FREEBSD",
        "OS_OPENBSD",
        "OS_WINDOWS",
        "OS_MACOS",
    ] {
        for font in [false, true] {
            for frontend in [false, true] {
                let mut facts: std::collections::BTreeMap<String, bool> = [
                    "OS_LINUX",
                    "OS_FREEBSD",
                    "OS_OPENBSD",
                    "OS_WINDOWS",
                    "OS_MACOS",
                ]
                .into_iter()
                .map(|name| (name.into(), name == os))
                .collect();
                facts.insert("HAVE_FONTCONFIG_TEST_OWNER".into(), font);
                facts.insert("HAVE_FRONTEND_WARN_TEST_OWNER".into(), frontend);
                let selected = graph.select(&facts).unwrap();
                let base = usize::from(["OS_LINUX", "OS_FREEBSD"].contains(&os))
                    + 2 * usize::from(os == "OS_LINUX");
                let count = base
                    + usize::from(os == "OS_LINUX")
                        * (2 * usize::from(font) + usize::from(frontend));
                assert_eq!(selected.targets.len(), count, "{os}/{font}/{frontend}");
                assert_eq!(
                    selected.tests.len(),
                    count + usize::from(os == "OS_LINUX" && font)
                );
            }
        }
    }
}
