# macOS Qt contract artifacts (#5008, slice c)

`ci/macos_qt_artifact.py` prepares the four contracts selected by the conditional
`Check Published Qt SDK` step in `rust-tests-macos`. It adds their target
executions to the existing `ci/macos_test_artifact.py` bundle. The workflow switch
remains separate: this infrastructure alone changes no hosted job.

```sh
uv run --no-project ci/macos_test_artifact.py build --output build_macos_tests
uv run --no-project ci/macos_qt_artifact.py build --work /tmp/obs-qt-producer --bundle build_macos_tests
# On macOS, extract the bundle into the same real checkout path, then:
uv run --no-project ci/macos_test_artifact.py run --manifest manifest.json
```

Use a fresh work directory for each preparation. The helper downloads and verifies
the Qt SDKs, asks soldr for its LLVM/Apple SDK drivers, cross-builds static cmocka,
and runs the original Rust tests on Linux with Darwin target selection. All graph,
generated-code, target-count, CMake configuration and CTest property assertions
still execute. Calls that would run target binaries instead record pending
executions. A successful preparation receipt proves preparation only. The macOS
manifest runner must execute every record and check its exit status; it preserves
the recorded working directories, environment, arguments and timeouts. No compiler,
Cargo, CMake build, or Qt generator is invoked by those runtime entries.

| Rust contract | Native sources and runtime coverage | Qt / libobs linkage |
| --- | --- | --- |
| `native_test_graph_generates_qt_metaobjects` | Generated `answer.hpp`, `answer.cpp`, `twin.cpp`, `answer.ui`, `answer.qrc`, `payload.txt`; both native and CMake builds. The answer checks the metaobject, invokable return value, generated form/header precedence and embedded resource; CMake also runs twin. | Core, Gui, Widgets; Linux moc/uic/rcc. No libobs. |
| `canonical_remux_queue_runs_unchanged` | Darwin selects `test_close_top_level`, `test_frontend_api_call`, `test_profile_import`, `test_remux_queue_model`, `test_settings_track_rows`, plus source-present `test_ws_image_mime` and `test_ws_session_duration`. Sources come unchanged from `test/cmocka/build/test-graph.json`: `test/cmocka/test_<name>.cpp`, `frontend/utility/RemuxQueueModel.cpp.hpp`, its moc header, the settings `.ui`, and the graph's source-only WebSocket helpers. The original CMake subset and all its property/inventory assertions remain. | Core, Gui, Widgets and static cmocka. These selected targets do not link libobs. Qt Test is absent from the pinned published SDK, so its original optional backtab guard remains false; Linux-only graph cases retain their original host guards. |
| `canonical_frontend_product_and_meter_tests_run_unchanged` | `test/cmocka/test_product_name.c`, `frontend/utility/product-name.c.h` and `test/cmocka/test_volume_meter_snapshot.cpp`; both native and CMake consumers. | Static cmocka and pthread; no Qt or libobs runtime. |
| `canonical_frontend_utility_tests_run_unchanged` | Original named frontend utility graph, selected for Darwin by `native_test_facts`; unchanged C tests plus their graph-listed frontend, host-policy and utility sources. Both native and CMake consumers retain the exact target/test inventory assertions. `graph.json` in the payload records the full source inventory. | Static cmocka, libcaption and system math as selected by the graph. Existing core-header interfaces remain headers only; no substituted libobs or installed libobs is linked. |

The remux inventory assertion also accounts for the two source-only WebSocket
owners that `validation::build_qt_tests` already discovers. It still checks exact
equality, and those helpers remain in the runtime payload. No WebSocket source,
public header, or existing C test is modified.

The SDK pins match the existing job and cross-compile host-tool pins:

| Input | URL | SHA-256 |
| --- | --- | --- |
| Darwin Qt 6.11.1 universal2, deployment target 11.0 | <https://github.com/obs-rust/obs-studio-libs/releases/download/qt6-6.11.1/qt6-6.11.1-darwin-universal2-11.0-release.tar.zst> | `85093eae764f83d167f8850c5b057c0ff5d2f928ad7d49fe2544ce0ca20074b0` |
| Linux x86-64 Qt 6.11.1 host tools | <https://github.com/obs-rust/obs-studio-libs/releases/download/qt6-6.11.1/qt6-6.11.1-linux-x86_64-glibc2.28-release.tar.zst> | `f0edfc3432bd57058bbb9a2cf18aacceb55a2564b018346778c34eb0e3369dbf` |
| cmocka 1.1.7 source, compiled for Darwin on Linux | <https://cmocka.org/files/1.1/cmocka-1.1.7.tar.xz> | `810570eb0b8d64804331f82b29ff47c790ce9cd6b163e98d47a4807047ecad82` |

`CMakePresets.json` additionally pins obs-deps' macOS Qt package from
`obs-rust/obs-deps` release `2026-10-25`, hash
`322cf861992c957f5fd2b3ec43ce42eebdb7162cce7f81f74c651b184e67adcd`.
This producer uses the published SDK that the original integration job tests.

The runtime closure contains arm64 Core, Gui, Widgets frameworks and the minimal
platform plugin. Their load names already use `@rpath`; the producer supplies
relative `@executable_path` rpaths for each native/CMake layout and rejects
unbundled load paths. macOS system dependencies include libc++, libSystem,
Objective-C, zlib and the Apple frameworks exported by the Qt interfaces (including
IOKit, DiskArbitration, UniformTypeIdentifiers, AppKit, CoreFoundation and
Foundation). No Apple SDK or Linux host tools are needed by the consumer.

The original arm64 Qt slices keep their ad-hoc signatures. soldr's ld64.lld signs
each new arm64 executable. The shared verifier checks every SHA-256 CodeDirectory
page, permits Apple's zero signature padding, and retains execute-only checking
by default; Qt dependency checks explicitly permit dylibs/bundles. Checksums cover
the complete transported payload. Missing contracts, missing/corrupt files,
unsigned or wrong-architecture output, unknown runtime properties and producer
load paths fail preparation. Hosted macOS execution and timing remain unverified
until the complete run-only workflow is activated.
