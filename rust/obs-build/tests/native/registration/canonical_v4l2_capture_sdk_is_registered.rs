use super::*;
use obs_build::validation::graph::Graph;

const CASES: [&str; 5] = ["test_v4l2_controls", "test_v4l2_corrupt_frame", "test_v4l2_dqbuf_error", "test_v4l2_fd_cloexec", "test_v4l2_framerate"];
const EXPECTED: &str = r#"{
  "test_v4l2_controls": {
    "sources": [
      "test/cmocka/test_v4l2_controls.c",
      "plugins/linux-v4l2/v4l2-controls.c.h",
      "plugins/linux-v4l2/v4l2-decoder.c.h",
      "plugins/linux-v4l2/v4l2-frame-action.c.h",
      "plugins/linux-v4l2/v4l2-framerate.c.h",
      "plugins/linux-v4l2/v4l2-helpers.c.h",
      "plugins/linux-v4l2/v4l2-input-caps.c.h",
      "plugins/linux-v4l2/v4l2-busy.c.h",
      "plugins/linux-v4l2/v4l2-input.c.h"
    ],
    "includes": [
      ".",
      "plugins/linux-v4l2"
    ],
    "watch_inputs": [
      "plugins/linux-v4l2/build/linux-v4l2-nix.pch.h"
    ],
    "header_packages": [
      "libv4l2"
    ],
    "libraries": [
      "OBS::libobs",
      "FFmpeg::avcodec",
      "FFmpeg::avformat",
      "FFmpeg::avutil",
      "OBS::native-test-cmocka"
    ],
    "requires": {
      "all": [
        "OS_LINUX",
        "HAVE_OBS_V4L2",
        "HAVE_V4L2_CAPTURE_SDK"
      ]
    },
    "definitions": [
      "HAVE_UDEV=1"
    ]
  },
  "test_v4l2_corrupt_frame": {
    "sources": [
      "test/cmocka/test_v4l2_corrupt_frame.c",
      "plugins/linux-v4l2/v4l2-controls.c.h",
      "plugins/linux-v4l2/v4l2-decoder.c.h",
      "plugins/linux-v4l2/v4l2-frame-action.c.h",
      "plugins/linux-v4l2/v4l2-framerate.c.h",
      "plugins/linux-v4l2/v4l2-helpers.c.h",
      "plugins/linux-v4l2/v4l2-input-caps.c.h",
      "plugins/linux-v4l2/v4l2-busy.c.h",
      "plugins/linux-v4l2/v4l2-input.c.h"
    ],
    "includes": [
      ".",
      "plugins/linux-v4l2"
    ],
    "watch_inputs": [
      "plugins/linux-v4l2/build/linux-v4l2-nix.pch.h"
    ],
    "header_packages": [
      "libv4l2"
    ],
    "libraries": [
      "OBS::libobs",
      "FFmpeg::avcodec",
      "FFmpeg::avformat",
      "FFmpeg::avutil",
      "OBS::native-test-cmocka"
    ],
    "requires": {
      "all": [
        "OS_LINUX",
        "HAVE_OBS_V4L2",
        "HAVE_V4L2_CAPTURE_SDK"
      ]
    }
  },
  "test_v4l2_dqbuf_error": {
    "sources": [
      "test/cmocka/test_v4l2_dqbuf_error.c",
      "plugins/linux-v4l2/v4l2-busy.c.h",
      "plugins/linux-v4l2/v4l2-controls.c.h",
      "plugins/linux-v4l2/v4l2-decoder.c.h",
      "plugins/linux-v4l2/v4l2-frame-action.c.h",
      "plugins/linux-v4l2/v4l2-framerate.c.h",
      "plugins/linux-v4l2/v4l2-helpers.c.h",
      "plugins/linux-v4l2/v4l2-input-caps.c.h",
      "plugins/linux-v4l2/v4l2-input.c.h"
    ],
    "includes": [
      ".",
      "plugins/linux-v4l2"
    ],
    "watch_inputs": [
      "plugins/linux-v4l2/build/linux-v4l2-nix.pch.h"
    ],
    "header_packages": [
      "libv4l2"
    ],
    "libraries": [
      "OBS::libobs",
      "FFmpeg::avcodec",
      "FFmpeg::avformat",
      "FFmpeg::avutil",
      "OBS::native-test-cmocka"
    ],
    "requires": {
      "all": [
        "OS_LINUX",
        "HAVE_OBS_V4L2",
        "HAVE_V4L2_CAPTURE_SDK"
      ]
    }
  },
  "test_v4l2_fd_cloexec": {
    "sources": [
      "test/cmocka/test_v4l2_fd_cloexec.c",
      "plugins/linux-v4l2/v4l2-busy.c.h",
      "plugins/linux-v4l2/v4l2-controls.c.h",
      "plugins/linux-v4l2/v4l2-decoder.c.h",
      "plugins/linux-v4l2/v4l2-frame-action.c.h",
      "plugins/linux-v4l2/v4l2-framerate.c.h",
      "plugins/linux-v4l2/v4l2-helpers.c.h",
      "plugins/linux-v4l2/v4l2-input-caps.c.h",
      "plugins/linux-v4l2/v4l2-input.c.h"
    ],
    "includes": [
      ".",
      "plugins/linux-v4l2"
    ],
    "watch_inputs": [
      "plugins/linux-v4l2/build/linux-v4l2-nix.pch.h"
    ],
    "header_packages": [
      "libv4l2"
    ],
    "libraries": [
      "OBS::libobs",
      "FFmpeg::avcodec",
      "FFmpeg::avformat",
      "FFmpeg::avutil",
      "OBS::native-test-cmocka"
    ],
    "requires": {
      "all": [
        "OS_LINUX",
        "HAVE_OBS_V4L2",
        "HAVE_V4L2_CAPTURE_SDK"
      ]
    }
  },
  "test_v4l2_framerate": {
    "sources": [
      "test/cmocka/test_v4l2_framerate.c",
      "plugins/linux-v4l2/v4l2-busy.c.h",
      "plugins/linux-v4l2/v4l2-controls.c.h",
      "plugins/linux-v4l2/v4l2-decoder.c.h",
      "plugins/linux-v4l2/v4l2-frame-action.c.h",
      "plugins/linux-v4l2/v4l2-framerate.c.h",
      "plugins/linux-v4l2/v4l2-helpers.c.h",
      "plugins/linux-v4l2/v4l2-input-caps.c.h",
      "plugins/linux-v4l2/v4l2-input.c.h"
    ],
    "includes": [
      ".",
      "plugins/linux-v4l2"
    ],
    "watch_inputs": [
      "plugins/linux-v4l2/build/linux-v4l2-nix.pch.h"
    ],
    "header_packages": [
      "libv4l2"
    ],
    "libraries": [
      "OBS::libobs",
      "FFmpeg::avcodec",
      "FFmpeg::avformat",
      "FFmpeg::avutil",
      "OBS::native-test-cmocka"
    ],
    "requires": {
      "all": [
        "OS_LINUX",
        "HAVE_OBS_V4L2",
        "HAVE_V4L2_CAPTURE_SDK"
      ]
    }
  }
}"#;

#[test]
fn canonical_v4l2_capture_sdk_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value = serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap()).unwrap();
    let expected: serde_json::Value = serde_json::from_str(EXPECTED).unwrap();
    for name in CASES {
        assert_eq!(raw["targets"][name], expected[name], "{name}");
        let timeout = if name == "test_v4l2_dqbuf_error" { 90 } else { 60 };
        assert_eq!(raw["tests"][name], serde_json::json!({"target": name, "requires": expected[name]["requires"], "timeout_seconds": timeout}), "{name}");
        let fragment = fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
    }
}

#[test]
fn canonical_v4l2_capture_sdk_guards() {
    let expected: serde_json::Value = serde_json::from_str(EXPECTED).unwrap();
    let mut tests = serde_json::Map::new();
    for name in CASES {
        tests.insert(name.into(), serde_json::json!({"target": name, "requires": expected[name]["requires"]}));
    }
    let graph = Graph::parse(&serde_json::json!({"schema": 1, "targets": expected, "tests": tests}).to_string()).unwrap();
    for os in ["OS_LINUX", "OS_FREEBSD", "OS_OPENBSD", "OS_WINDOWS", "OS_MACOS"] {
        for owner in [false, true] {
            for sdk in [false, true] {
                let facts = [("OS_LINUX".into(), os == "OS_LINUX"), ("HAVE_OBS_V4L2".into(), owner), ("HAVE_V4L2_CAPTURE_SDK".into(), sdk)].into();
                let selected = graph.select(&facts).unwrap();
                let count = if os == "OS_LINUX" && owner && sdk { 5 } else { 0 };
                assert_eq!(selected.targets.len(), count);
                assert_eq!(selected.tests.len(), count);
            }
        }
    }
}
