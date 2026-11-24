//! obs-studio-rust integration tests: cli (one module per former tests/*.rs file, #968).

mod cli_json;
mod ipv6;
#[cfg(target_os = "linux")]
#[path = "../../../obs-build/tests/locale_manifest_contract.rs"]
mod locale_manifest_contract;
#[cfg(feature = "bundled-native")]
mod mcp_stdio;
mod protocol;
#[cfg(target_os = "linux")]
#[path = "../../../obs-vst3/src/ffi/run_loop_registry.rs"]
mod vst3_editor_run_loop;
mod vst3_filter;
mod vst3_host;
mod vst3_scanner;

#[cfg(feature = "bundled-native")]
#[path = "../../../obs-sys/tests/native.rs"]
mod bundled_libobs;
#[cfg(feature = "bundled-native")]
#[path = "../../../obs-mcp/tests/libobs_backend.rs"]
mod mcp_libobs_backend;
#[cfg(feature = "bundled-native")]
#[path = "../../../obs-mcp/tests/libobs_video.rs"]
mod mcp_libobs_video;
#[path = "../../../obs-build/src/platform/msvc_response.rs"]
mod msvc_response;
#[path = "../../../obs-build/tests/native.rs"]
mod native_build;
#[cfg(feature = "libobs-integration")]
#[path = "../../../obs-build/tests/libobs.rs"]
mod native_libobs;
#[path = "../../../obs-build/tests/qt_sdk.rs"]
mod qt_sdk;
#[path = "../../../obs-build/src/platform/shared_library.rs"]
mod shared_library;
