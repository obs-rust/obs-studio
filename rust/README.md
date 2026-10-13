# Rust ports of OBS Studio internals

OBS keeps its public API (libobs C API/ABI, obs-websocket v5, frontend and
scripting APIs) while its internals are rewritten in Rust. Every port follows
[docs/rust-port/testing-policy.md](../docs/rust-port/testing-policy.md).

## Layout

| Path | What |
|---|---|
| `obs-util/` | Ports of `libobs/util/*`: safe cores at the crate root, C ABI shims in `src/ffi/` |
| `obs-graphics/` | Ports of `libobs/graphics/*` |
| `obs-codec/` | Ports of the libobs codec bitstream helpers (`obs-nal.c`, `obs-avc.c`, `obs-hevc.c`, `obs-av1.c`) |
| `obs-media-io/` | Ports of `libobs/media-io/*` |
| `obs-c-oracle/` | Test-only: original C sources compiled with `oracle_` symbols for layout and differential tests |
| `libobs-rust/` | The single staticlib linked into libobs when `ENABLE_RUST_LIBOBS=ON` |
| `obs-studio-rust/` | Standalone CLI that starts, stops and inspects OBS streaming over obs-websocket v5. Named after the product, with no release tags of its own (`docs/rust-port/naming.md`) |
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
| `libobs/util/buffered-file-serializer.c` | `obs-util::buffered_file_serializer` | `SERIALIZE_SEEK_END` subtracts from the virtual write position. Chunks under 64 KiB stay off disk until free, a discontinuous seek, or `chunk_size`. A flushed chunk goes out through `File::write`, so those bytes can show up before close. C's `fwrite` can keep them in the stdio buffer until `fclose`. Debug and error `blog` lines are not emitted. A null serializer pointer to `buffered_file_serializer_free` returns. A null data pointer with a non-zero size returns 0. |
| `libobs/util/crc32.c` | `obs-util::crc32` | no intentional differences |
| `libobs/util/config-file.c` | `obs-util::config_file` | Null `config`, `section` or `name` arguments are null-safe in the shim (C dereferences or stores them). Rust never re-locks the config mutex where C relies on `PTHREAD_MUTEX_RECURSIVE`. Follows the #70 save fixes (failed flush is `CONFIG_ERROR`, empty config saves). Windows non-UTF-8 paths substitute U+FFFD like libobs, via a different maximal-invalid-subpart rule. |
| `libobs/util/pipe.c`, `pipe-posix.c`, `pipe-windows.c` | `obs-util::pipe` | Swapped on every OS. Null command, args or arg are null-safe in the shim (C would crash). A signaled child returns its pid from `os_process_pipe_destroy`, as in C. On Windows, Win32 is called as in C (not `std::process`), and handles C leaks when creating a pipe fails half-way are closed. |
| `libobs/util/cf-tokenizer.c` (extracted from `cf-lexer.c`) | `obs-util::cf_tokenizer` | Reads NUL where C reads past the terminator (text ending in a line splice inside a comment or string; `\x`/octal escapes skipping past a literal's NUL). A NULL `cf_literal_to_str` literal returns NULL (C dereferences it). The preprocessor in `cf-lexer.c` stays in C. |
| `libobs/util/darray.h` (header-inline, not swapped) | `obs-util::darray` | Layout and parity only; the `struct darray` layout is the contract. |
| `libobs/util/utf8.c` | `obs-util::utf8` | Swapped on every OS. The Windows branch (`platform::windows::utf8`) calls `MultiByteToWideChar`/`WideCharToMultiByte` with the C's arguments; sizes above `i32::MAX` give 0 instead of a negative `int`. Shims are hidden like the C original (not exported by libobs). |
| `libobs/util/lexer.c` | `obs-util::lexer` | Header-inline helpers stay C; `cf-lexer.c` and `cf-parser.c` are not ported. Follows the C fix for #48 (an empty strref sorts before a non-empty one in both argument orders). |
| `libobs/util/text-lookup.c` | `obs-util::text_lookup` | Rust does not crash on a value whose opening quote ends the line; C wraps the length to `SIZE_MAX`. |
| `libobs/util/dstr.c` | `obs-util::dstr` | printf, wide-char and conversion functions are extracted to `util/dstr-libc.c` and stay C. Follows the C fixes for #46 (`dstr_insert_ch` overrun) and #47 (an empty `find` in `dstr_replace` is a no-op). |
| `libobs/util/bmem.c` | `obs-util::bmem` | no intentional differences; 32-byte alignment and `bnum_allocs` accounting as in C |
| `libobs/graphics/vec2.c` | `obs-graphics::vec2` | `vec2_norm` leaves dst unchanged for zero/NaN length, as in C; header-inline helpers stay C |
| `libobs/util/base.c` | `obs-util::base` | `blog`, `blogva`, and `bcrash` stay in `util/base-variadic.c` (stable Rust cannot define C variadics). The Rust core owns the handler slots. Updates are mutex-ordered and the lock is dropped before the handler runs; C used plain stores. |
| `libobs/graphics/axisang.c` | `obs-graphics::axisang` | No intentional differences. `acos` is the C library's `acosf`. Header-inline helpers stay C. |
| `libobs/graphics/quat.c` | `obs-graphics::quat` | No intentional differences. `quat_dot` sums in the SSE order. `quat_get_dir` computes the Z axis of `matrix3_from_quat` (that file stays C). `quat_from_matrix3` reads the `matrix3` as a `matrix4`, including each axis `w` lane, as the C cast does. Header-inline helpers stay C. |
| `libobs/graphics/matrix3.c` | `obs-graphics::matrix3` | No intentional differences. `matrix3_inv` reimplements the `matrix4_inv` cofactor path (including the `0.0005` determinant cutoff and the "write nothing, then clear `t.w`" failure). The transpose copies `z.w` into every axis `w` lane, as the SSE shuffles do. Header-inline helpers stay C. |
| `libobs/graphics/matrix4.c` | `obs-graphics::matrix4` | No intentional differences. `matrix4_mul` dots in the SSE `vec4_dot` order. `matrix4_transpose` moves lanes, matching the SSE unpacks bit for bit, so parity is exact (0 ULP) even though the C transpose is SIMD. `|det| < 0.0005` leaves `dst` unchanged. `matrix4_mul_4x3_only` is not exported. Header-inline helpers stay C. |
| `libobs/graphics/plane.c` | `obs-graphics::plane` | No intentional differences. `vec3_cross` and `vec3_norm` follow the header SSE, including a zero normal when the length squared is not positive. `plane_transform` moves the normal as a point. `plane_intersection_line` does not write `*t` on a miss. Header-inline helpers stay C. |
| `libobs/graphics/bounds.c` | `obs-graphics::bounds` | `bounds_under_plane` and `bounds_plane_test` set the corner `w` lanes to 0. C leaves them uninitialized, and `vec3_dot` reads them, so those two are not compared to the oracle. The safe `plane_test` returns `BOUNDS_OUTSIDE` (1), `BOUNDS_INSIDE` (2), or `BOUNDS_PARTIAL` (3); the C `bool` return collapses every one of those to 1, and the shim does too. `bounds_intersects_obb` uses a zero matrix when `|det| < 0.0005`; C leaves that inverse uninitialized, and parity skips those matrices. `vec3_min` / `vec3_max` follow SSE (the second lane unless the comparison holds). A ray or line miss does not write `*t`. Index 8 of `bounds_get_point` writes a corner; `i > 8` writes nothing. Header-inline helpers stay C. |
| `libobs/graphics/math-extra.c` | `obs-graphics::math_extra` | `polar_to_cart` and `cart_to_polar` set `w` to 0. C leaves that lane uninitialized except when `cart_to_polar` zeros a length within `EPSILON`, so parity compares xyz only. `polar_to_norm` does not write `w`. `vec3_close` is one-sided, so `calc_torque` copies `v2` when every lane of `v1 - v2` is `< EPSILON`. The shim re-reads `v1` after storing the scaled step, so `dst == v1` matches C. `rand_float` calls the C library `rand` / `RAND_MAX`. Header-inline `get_percentage` and `get_percentagei` stay C. |
| `libobs/graphics/vec3.c` | `obs-graphics::vec3` | No intentional differences. `Vec3` keeps the SSE `w` lane because `vec3_dot` multiplies it. `vec3_rand` calls `rand_float` from the math-extra port. Header-inline helpers stay C. |
| `libobs/util/threading-posix.c`, `threading-windows.c` | `obs-util::threading` | Header-inline atomics stay C. Unix keeps the pthread event and the native semaphore (`sem_t` / Mach). Windows uses `CreateEvent` / `CreateSemaphore`, including the manual-reset wake of every waiter and a post at `0x7FFFFFFF` returning -1. `os_set_thread_name` raises `0x406D1388` (caught, MSVC only) and calls `SetThreadDescription`. A NULL out-pointer after a successful create returns `EINVAL` and the kernel object is closed; C writes through the pointer. The boxed handle is not the raw `HANDLE`; both are opaque and neither allocates with `bmalloc`. |
| `libobs/util/task.c` | `obs-util::task` | A NULL `os_task_t` aborts when it runs (C calls the NULL pointer). A NULL queue to `os_task_queue_queue_task` or `os_task_queue_inside` returns false (C dereferences it). The worker is a Rust thread and waits on the ported event and semaphore. |
| `libobs/util/profiler.c` | `obs-util::profiler` | Names are pointer identity. Print and the CSV/gzip dump stay in `util/profiler-io.c` (zlib and dstr printf). `profile_store_name` stays in `util/profiler-variadic.c` and pushes through hidden `profiler_name_store_push`. The root mutex is process-lifetime, so `profiler_free` does not make a later `profiler_start` undefined. Tier 3 feeds both sides one scripted `os_gettime_ns`, so the time buckets match that clock. |
| `libobs/util/profiler-snapshot.c` (extracted from `profiler.c`) | `obs-util::profiler_snapshot` | Snapshot accessors and `profile_snapshot_free`. The `profiler-snapshot.h` struct layout is the contract. A NULL `snap` to `profiler_snapshot_filter_roots`, or a NULL callback with entries to visit, does nothing (C dereferences it). |
| `libobs/graphics/vec4.c` | `obs-graphics::vec4` | No intentional differences. `Vec4::dot` sums in the SSE `vec4_dot` order. Header-inline helpers stay C. |
| `libobs/obs-nal.c` | `obs-codec::nal` | The C word-at-a-time search is replaced by a byte scan with the same result at any alignment; a start code in the last three bytes is not reported, as in C. A range starting within 3 bytes of address 0, such as `(NULL, NULL)`, returns `end`; C computes `end - 3`, wraps, and reads address 0. |
| `libobs/media-io/video-fourcc.c` | `obs-media-io::video_fourcc` | No intentional differences. `enum video_format` crosses the C ABI as a `c_int`, like `serialize_seek_type` in obs-util. |
| `libobs/media-io/video-matrices.c` | `obs-media-io::video_matrices` | No intentional differences. Each call computes its matrix with the C's operations in the same order, instead of filling a table on first use behind an unsynchronized `static bool`. `enum video_colorspace` and `enum video_range_type` cross the C ABI as `c_int`s. |
| `libobs/obs-avc.c` | `obs-codec::avc` | Uses the shared NAL walk and packet helpers in `obs-codec::nal`. `obs_parse_avc_packet` keeps the `long` reference count in front of the data (4 bytes on Windows, 8 elsewhere); SPS/PPS sizes wrap to 16 bits in the header record, as in C. NULL data with size 0 gives empty results (the obs-nal NULL-range read in C). Fuzzed differentially against the C oracle by `rust/obs-codec/fuzz` target `avc_diff`; NULL/near-0 ranges are excluded (C UB). |
| `libobs/obs-hevc.c` | `obs-codec::hevc` | Uses the shared NAL walk and packet helpers in `obs-codec::nal`. NULL data with size 0 gives empty results (the obs-nal NULL-range read in C). With `ENABLE_HEVC=OFF` and `ENABLE_RUST_LIBOBS=ON`, libobs still exports the four `obs_*hevc*` functions, which the C build leaves out. Fuzzed differentially against the C oracle by `rust/obs-codec/fuzz` target `hevc_diff`; NULL/near-0 ranges are excluded (C UB). |
| `libobs/obs-av1.c` | `obs-codec::av1` | No intentional differences: the port follows obs-av1.c with its OBU bounds fix (#103), so an OBU never extends past the buffer and any bytes are a valid input. Differential fuzz target: `obs-codec/fuzz` (`av1_differential`). |
| `libobs/media-io/video-io.h` and `frame-rate.h` (header-inline, not swapped) | `obs-media-io::video_io`, `obs-media-io::frame_rate` | No intentional differences. Layout and parity only; the `struct video_data`, `struct video_output_info`, `struct video_scale_info` and `struct media_frames_per_second` layouts are the contract. `ffi::video_io` gives enum values outside the enums the C's result. |
| `libobs/media-io/video-io.c` | `obs-media-io::video_output` | Frames are zeroed; C's `bmalloc` leaves them uninitialized. `close` on a divisor child stops the root and frees only the child; C also frees the parent's cache and mutexes. The stop flag is atomic; C uses a plain `bool`. `connect(NULL)` returns false; C crashes in `get_root`. A null `get_skipped_frames` or `get_total_frames` returns 0; C dereferences the handle. A null `info` or out-pointer to `open` returns `VIDEO_OUTPUT_INVALIDPARAM`; C crashes. A null frame pointer to `lock_frame` returns true and writes nothing; C crashes in `memcpy`. `video_scaler_*` stays the FFmpeg C file. |
| `libobs/media-io/audio-io.h` inlines (header-inline, not swapped) | `obs-media-io::audio_io` | No intentional differences. Layout and parity only; the `struct audio_data`, `struct audio_output_data`, `struct audio_output_info` and `struct audio_convert_info` layouts are the contract. `ffi::audio_io` gives enum values outside the enums the C's result (`enum speaker_layout` skips 7). `audio_frames_to_ns` and `ns_to_audio_frames` call `obs-util::util_uint64::mul_div64` and panic where it does: a sample rate of 0 to `audio_frames_to_ns`, and on MSVC x64 a result past 64 bits. |
| `libobs/util/util_uint64.h` (header-inline, not swapped) | `obs-util::util_uint64` | No intentional differences. Like the header, `mul_div64` does exact 128-bit math on MSVC x64 and `(num / div) * mul + (rem * mul) / div` in wrapping u64 elsewhere, so the two differ once `rem * mul` overflows. Panics where the C is UB or faults: `div == 0`, and on MSVC x64 a quotient past 64 bits; parity excludes those. |
| `libobs/media-io/audio-math.h` (header-inline, not swapped) | `obs-media-io::audio_math` | No intentional differences. `mul_to_db` and `db_to_mul` call the platform `log10f`/`powf` like the header, so parity is exact bit equality on each platform. |
| `libobs/media-io/video-frame.c` (+ `video-frame.h` inlines, not swapped) | `obs-media-io::video_frame` | No intentional differences. Only `video_frame_init` and `video_frame_copy` are swapped; `video_frame_get_linesizes` / `_get_plane_heights` have no C symbol. V210 keeps the C's `ALIGN` to 48, which only rounds to a power of two (width 1280 gives linesize 3424, not 3456). The `u32` math wraps as in C. `VIDEO_FORMAT_NONE`, an unknown format and a zero width or height still reach `bmalloc(0)`, which crashes. `enum video_format` crosses the C ABI as a `c_int`. |
| `libobs/media-io/format-conversion.c` | `obs-media-io::format_conversion` | No intentional differences. The C's SSE2 is integer only, so the port is scalar and parity is byte-exact. The planes are `Source`/`Sink` traits: byte slices panic where the C reads or writes out of bounds, and the shims read the caller's raw pointers at the C's offsets. Quirks kept: `decompress_422` reads and writes twice each row's width, so rows spill into the next. |
| `libobs/util/platform-nix.c` | `obs-util::platform::unix::nix` | Swapped on macOS, Linux, and FreeBSD. `platform-cocoa.m` still provides the Apple clock, paths, CPU, memory, and inhibit. `os_inhibit_sleep_*` is not ported: on Linux and FreeBSD it stays C in `util/platform-nix-inhibit.c`, built with the option ON too, so it still uses the portal and session D-Bus inhibitors (`platform-nix-portal.c`, `platform-nix-dbus.c`) and falls back to `xdg-screensaver reset`. `os_opendir` copies the path (C keeps the caller's pointer). Null pointers that would crash return an error or null. Glob paths are copies freed by `os_globfree`. `os_dir` and `os_cpu_usage_info` are `Box` values, not `bmalloc`. Core-count and total-memory caches take a mutex. A missing `$HOME` still `bcrash`es in the shim. FreeBSD total memory reads `hw.physmem`. |

## obs-studio-rust

`obs-studio-rust` is a small client for a running OBS (Tools → WebSocket
Server Settings must be enabled):

```sh
obs-studio-rust status                           # stream: live 00:01:02.345, ...
obs-studio-rust --password secret start          # or OBS_WEBSOCKET_PASSWORD=secret
obs-studio-rust --host 192.168.1.5 --port 4455 stop
obs-studio-rust --host ::1 status
obs-studio-rust --host '[::1]' status             # IPv6 brackets are optional
obs-studio-rust toggle
obs-studio-rust version
```

For scripts and agents, add `--json` before or after the command:

```sh
obs-studio-rust --json status
obs-studio-rust version --json
```

This prints one compact JSON value containing the server's full `responseData`,
including fields omitted from the default text output. A response without data
prints `null`; an empty object remains `{}`. Errors still go to stderr with a
nonzero exit status, so scripts should check the exit status as well as parse
stdout. Local `--help` and `--version` keep their text output.

Binaries are cross-compiled from Linux with
[cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild):

```sh
for t in x86_64-unknown-linux-musl x86_64-pc-windows-gnu x86_64-apple-darwin aarch64-apple-darwin; do
  cargo-zigbuild build --release --locked -p obs-studio-rust --target "$t"
done
```

Published binaries go on the app's `obs-studio-rust-v*` releases; the
`obs-stream-rust-v0.1.0` tag is retired.
