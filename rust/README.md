# Rust ports of OBS Studio internals

OBS keeps its public API (libobs C API/ABI, obs-websocket v5, frontend and
scripting APIs) while its internals are rewritten in Rust. Every port follows
[docs/rust-port/testing-policy.md](../docs/rust-port/testing-policy.md).

## Layout

| Path | What |
|---|---|
| `obs-util/` | Ports of `libobs/util/*`: safe cores at the crate root, C ABI shims in `src/ffi/` |
| `obs-graphics/` | Ports of `libobs/graphics/*` |
| `obs-c-oracle/` | Test-only: original C sources compiled with `oracle_` symbols for layout and differential tests |
| `libobs-rust/` | The single staticlib linked into libobs when `ENABLE_RUST_LIBOBS=ON` |
| `obs-stream-rust/` | Standalone CLI that starts, stops and inspects OBS streaming over obs-websocket v5 (released as `obs-stream-rust` binaries) |
| `tools/linux-validate/` | Docker harness that builds libobs with the Rust ports OFF and ON |
| `tools/tier2-validate/` | macOS/Windows Tier 2 (C tests plus exported symbols, OFF vs ON), used by CI |

## Running the tests

The fastest path is the local gate, which runs the same checks as CI's Linux jobs:

```sh
./lint          # clang-format, gersemi, rustfmt, clippy (~8 s)
./unittest      # Rust Tiers 1-3, then Tier 2 in Docker (~40 s warm)
uvx --from git+https://github.com/zackees/ci.yml@1a970f01574447d681571ff3d0b8de56cb983dcc ci-lint local-gate run
```

The last command stamps the commit, so CI skips the Linux Rust test and Tier 2 jobs for that PR.
The individual commands are:

```sh
soldr cargo test --workspace                               # Tiers 1-3
soldr cargo clippy --workspace --all-targets -- -D warnings
soldr cargo fmt --all --check
rust/tools/linux-validate/run.sh                           # Tier 2: C tests + ABI, OFF vs ON
rust/tools/linux-validate/run.sh --repeat 100 --tests 'test_threading|test_task'  # + flake check
```

CI runs these in `.github/workflows/build-project.yaml` (jobs `rust-tests`,
`rust-linux-tier2`, `rust-tier2`).

To build OBS itself with the Rust ports, configure with
`-DENABLE_RUST_LIBOBS=ON`; add `-DENABLE_UNIT_TESTS=ON` to build the cmocka
tests and run them with `ctest`.

## Ported so far

| C source | Rust | Notes |
|---|---|---|
| `libobs/util/bitstream.c` | `obs-util::bitstream` | C ABI keeps the `uint8_t pos` wrap past byte 255; the safe API does not wrap |
| `libobs/util/path-extension.c` (extracted from `platform.c`) | `obs-util::path_extension` | NULL `path` returns NULL in Rust (C dereferences it). |
| `libobs/util/array-serializer.c` | `obs-util::array_serializer` | `get_pos` returns `bytes.num`, not `cur_pos`, as in C. `serializer.h` is header-inline (layout test only). |
| `libobs/util/file-serializer.c` | `obs-util::file_serializer` | A null `path` to `file_output_serializer_init_safe` returns false. C would build a temp name from the extension and `os_unlink(NULL)` on free. On Windows, a failed seek (before the start of the file) keeps the position, where the MSVC CRT reports its read-ahead position. A failed safe save is not logged. |
| `libobs/util/crc32.c` | `obs-util::crc32` | no intentional differences |
| `libobs/util/darray.h` (header-inline, not swapped) | `obs-util::darray` | Layout and parity only; the `struct darray` layout is the contract. |
| `libobs/util/utf8.c` | `obs-util::utf8` | Swapped on non-Windows only; Windows keeps the C `MultiByteToWideChar` path. Shims are hidden like the C original (not exported by libobs). |
| `libobs/util/lexer.c` | `obs-util::lexer` | Header-inline helpers stay C; `cf-lexer.c` and `cf-parser.c` are not ported. Follows the C fix for #48 (an empty strref sorts before a non-empty one in both argument orders). |
| `libobs/util/text-lookup.c` | `obs-util::text_lookup` | Rust does not crash on a value whose opening quote ends the line; C wraps the length to `SIZE_MAX`. |
| `libobs/util/dstr.c` | `obs-util::dstr` | printf, wide-char and conversion functions are extracted to `util/dstr-libc.c` and stay C. Follows the C fixes for #46 (`dstr_insert_ch` overrun) and #47 (an empty `find` in `dstr_replace` is a no-op). |
| `libobs/util/bmem.c` | `obs-util::bmem` | no intentional differences; 32-byte alignment and `bnum_allocs` accounting as in C |
| `libobs/graphics/vec2.c` | `obs-graphics::vec2` | `vec2_norm` leaves dst unchanged for zero/NaN length, as in C; header-inline helpers stay C |
| `libobs/util/base.c` | `obs-util::base` | `blog`, `blogva`, and `bcrash` stay in `util/base-variadic.c` (stable Rust cannot define C variadics). The Rust core owns the handler slots. Updates are mutex-ordered and the lock is dropped before the handler runs; C used plain stores. |
| `libobs/graphics/vec3.c` | `obs-graphics::vec3` | No intentional differences. `Vec3` keeps the SSE `w` lane because `vec3_dot` multiplies it. `vec3_rand` calls libobs `rand_float`, which stays C. Header-inline helpers stay C. |
| `libobs/graphics/vec4.c` | `obs-graphics::vec4` | No intentional differences. `Vec4::dot` sums in the SSE `vec4_dot` order. Header-inline helpers stay C. |

## obs-stream-rust

`obs-stream-rust` is a small client for a running OBS (Tools → WebSocket
Server Settings must be enabled):

```sh
obs-stream-rust status                           # stream: live 00:01:02.345, ...
obs-stream-rust --password secret start          # or OBS_WEBSOCKET_PASSWORD=secret
obs-stream-rust --host 192.168.1.5 --port 4455 stop
obs-stream-rust toggle
obs-stream-rust version
```

Release binaries are cross-compiled from Linux with
[cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild):

```sh
for t in x86_64-unknown-linux-musl x86_64-pc-windows-gnu x86_64-apple-darwin aarch64-apple-darwin; do
  cargo-zigbuild build --release --locked -p obs-stream-rust --target "$t"
done
```
