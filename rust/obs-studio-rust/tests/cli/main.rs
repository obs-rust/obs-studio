//! obs-studio-rust integration tests: cli (one module per former tests/*.rs file, #968).

mod cli_json;
mod ipv6;
mod protocol;

#[cfg(feature = "bundled-native")]
#[path = "../../../obs-sys/tests/native.rs"]
mod bundled_libobs;
#[cfg(feature = "bundled-native")]
#[path = "../../../obs-mcp/tests/libobs_backend.rs"]
mod mcp_libobs_backend;
#[path = "../../../obs-build/tests/native.rs"]
mod native_build;
#[cfg(feature = "libobs-integration")]
#[path = "../../../obs-build/tests/libobs.rs"]
mod native_libobs;
#[path = "../../../obs-build/tests/qt_sdk.rs"]
mod qt_sdk;
#[path = "../../../obs-build/src/platform/shared_library.rs"]
mod shared_library;
