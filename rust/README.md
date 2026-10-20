# Rust ports of OBS Studio internals

OBS keeps its public API (libobs C API/ABI, obs-websocket v5, frontend and
scripting APIs) while its internals are rewritten in Rust. Every port follows
[docs/rust-port/testing-policy.md](../docs/rust-port/testing-policy.md).

## Layout

Which crate a C component's port goes in, and the repository layout policy,
are in [docs/rust-port/naming.md](../docs/rust-port/naming.md#repository-layout-1051).

| Path | What |
|---|---|
| `obs-util/` | Ports of `libobs/util/*`: safe cores at the crate root, C ABI shims in `src/ffi/` |
| `obs-graphics/` | Ports of `libobs/graphics/*` |
| `obs-codec/` | Ports of the libobs codec bitstream helpers (`obs-nal.c`, `obs-avc.c`, `obs-hevc.c`, `obs-av1.c`) |
| `obs-media-io/` | Ports of `libobs/media-io/*` |
| `obs-data/` | Port of `libobs/obs-data.c` |
| `obs-properties/` | Port of `libobs/obs-properties.c` |
| `obs-hotkey/` | Ports of `libobs/obs-hotkey-name-map.c`, `libobs/obs-nix-hotkey-state.c.h` and `libobs/obs-nix-portal-shortcut-ids.c.h` |
| `obs-callback/` | Port of `libobs/callback/calldata.c.h`, `callback/decl.c.h` and `callback/proc.c.h` |
| `obs-missing-files/` | Port of `libobs/obs-missing-files.c.h` |
| `obs-scene-blend/` | Port of `libobs/obs-scene-blend.c.h` |
| `obs-scene-bounds/` | Port of `libobs/obs-scene-bounds.c.h` |
| `obs-audio-monitoring/` | Ports of `libobs/audio-monitoring/*` (so far `pulse/pulseaudio-stream-flags.c.h`) |
| `obs-nvenc/`, `obs-nvenc-plugin/` | Port of the `plugins/obs-nvenc` CUDA/NVENC loader, and the staticlib that plugin links |
| `obs-gpu-status/`, `obs-gpu-status-frontend/` | GPU acceleration status (#846), and the staticlib the frontend links |
| `obs-c-oracle/` | Test-only: original C sources compiled with `oracle_` symbols for layout and differential tests |
| `libobs-rust/` | The single staticlib linked into `libobs-with-rust`, libobs's never-installed twin that every build makes (#1065) |
| `obs-studio-rust/` | Standalone CLI that starts, stops and inspects OBS streaming over obs-websocket v5. Named after the product, with no release tags of its own (`docs/rust-port/naming.md`) |
| `tools/linux-validate/` | Docker harness that builds libobs with the Rust ports OFF and ON |
| `tools/tier2-validate/` | macOS/Windows Tier 2 (C tests plus exported symbols, OFF vs ON), used by CI |

## Running the tests

PR CI is the merge gate, so opening a PR is enough. For faster feedback, the
local gate runs the same checks as CI's Linux jobs (optional):

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

Every OBS build also builds `libobs-with-rust` (the Rust ports in place of
the ported C, in `<build>/libobs-with-rust/`), but links, installs and packages
the C `libobs` (#1065). To run the cmocka tests (`-DENABLE_UNIT_TESTS=ON`)
against the Rust ports, put that directory first on `LD_LIBRARY_PATH`
(macOS: `DYLD_FRAMEWORK_PATH`; Windows: `PATH`), as Tier 2 does.
`ENABLE_RUST_LIBOBS` now only names the app OBS-Studio-Rust.

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
| `libobs/util/cf-parser.c.h` | `obs-util::cf_parser` | `cf_pass_pair` and `cf_adderror`, with no intentional behavior differences. Preserves token cursor position, source locations, sequential substitutions, null versus empty diagnostics, and bmalloc-owned error messages. Preprocessing and header-inline helpers remain C. |
| `libobs/util/cf-tokenizer.c` (extracted from `cf-lexer.c`) | `obs-util::cf_tokenizer` | Reads NUL where C reads past the terminator (text ending in a line splice inside a comment or string; `\x`/octal escapes skipping past a literal's NUL). A NULL `cf_literal_to_str` literal returns NULL (C dereferences it). The preprocessor in `cf-lexer.c` stays in C. |
| `libobs/util/darray.h` (header-inline, not swapped) | `obs-util::darray` | Layout and parity only; the `struct darray` layout is the contract. |
| `libobs/util/utf8.c` | `obs-util::utf8` | Swapped on every OS. The Windows branch (`platform::windows::utf8`) calls `MultiByteToWideChar`/`WideCharToMultiByte` with the C's arguments; sizes above `i32::MAX` give 0 instead of a negative `int`. Shims are hidden like the C original (not exported by libobs). |
| `libobs/util/lexer.c` | `obs-util::lexer` | Header-inline helpers stay C; `cf-lexer.c` remains C; the two `cf-parser.c` exports are ported separately. Follows the C fix for #48 (an empty strref sorts before a non-empty one in both argument orders). |
| `libobs/util/text-lookup.c` | `obs-util::text_lookup` | Rust does not crash on a value whose opening quote ends the line; C wraps the length to `SIZE_MAX`. |
| `libobs/util/dstr.c` | `obs-util::dstr` | printf, wide-char and conversion functions are extracted to `util/dstr-libc.c` and stay C. Follows the C fixes for #46 (`dstr_insert_ch` overrun) and #47 (an empty `find` in `dstr_replace` is a no-op). A source that points into the destination's own buffer (`dstr_ncat` and `dstr_cat_strref` with a pointer into `dst->array`, `dstr_cat_dstr(d, d)`, `dstr_copy` from inside `dst->array`) is undefined in C: C `brealloc`s and then `memcpy`s from the freed pointer, or copies overlapping ranges. Rust finds the source again after the grow and `memmove`s it, so the result is the source bytes as they were before the call (#1297). |
| `libobs/util/bmem.c` | `obs-util::bmem` | no intentional differences; 32-byte alignment and `bnum_allocs` accounting as in C |
| `libobs/graphics/vec2.c` | `obs-graphics::vec2` | `vec2_norm` leaves dst unchanged for zero/NaN length, as in C; header-inline helpers stay C |
| `libobs/util/base.c` | `obs-util::base` | `blog`, `blogva`, and `bcrash` stay in `util/base-variadic.c` (stable Rust cannot define C variadics). The Rust core owns the handler slots. Updates are mutex-ordered and the lock is dropped before the handler runs; C used plain stores. |
| `libobs/graphics/axisang.c` | `obs-graphics::axisang` | No intentional differences. `acos` is the C library's `acosf`. Header-inline helpers stay C. |
| `libobs/graphics/quat.c` | `obs-graphics::quat` | No intentional differences. `quat_dot` sums in the SSE order. `quat_get_dir` computes the Z axis of `matrix3_from_quat` (that file stays C). `quat_from_matrix3` reads the `matrix3` as a `matrix4`, including each axis `w` lane, as the C cast does. Header-inline helpers stay C. |
| `libobs/graphics/matrix3.c` | `obs-graphics::matrix3` | No intentional differences. `matrix3_inv` reimplements the `matrix4_inv` cofactor path (including the `0.0005` determinant cutoff and the "write nothing, then clear `t.w`" failure). The transpose copies `z.w` into every axis `w` lane, as the SSE shuffles do. Header-inline helpers stay C. |
| `libobs/graphics/matrix4.c` | `obs-graphics::matrix4` | No intentional differences. `matrix4_mul` dots in the SSE `vec4_dot` order. `matrix4_transpose` moves lanes, matching the SSE unpacks bit for bit, so parity is exact (0 ULP) even though the C transpose is SIMD. `|det| < 0.0005` leaves `dst` unchanged. `matrix4_mul_4x3_only` is not exported. Header-inline helpers stay C. |
| `libobs/graphics/plane.c` | `obs-graphics::plane` | No intentional differences. `vec3_cross` and `vec3_norm` follow the header SSE, including a zero normal when the length squared is not positive. `plane_transform` moves the normal as a point. `plane_intersection_line` does not write `*t` on a miss. Header-inline helpers stay C. |
| `libobs/graphics/bounds.c` | `obs-graphics::bounds` | `bounds_under_plane` and `bounds_plane_test` set the corner `w` lanes to 0. C leaves them uninitialized, and `vec3_dot` reads them, so those two are not compared to the oracle. The safe `plane_test` returns `BOUNDS_OUTSIDE` (1), `BOUNDS_INSIDE` (2), or `BOUNDS_PARTIAL` (3); the C `bool` return collapses every one of those to 1, and the shim does too. `bounds_intersects_obb` uses a zero matrix when `|det| < 0.0005`; C leaves that inverse uninitialized, and parity skips those matrices. `vec3_min` / `vec3_max` follow SSE (the second lane unless the comparison holds). A ray or line miss does not write `*t`. Index 8 of `bounds_get_point` writes a corner; `i > 8` writes nothing. Header-inline helpers stay C. |
| `libobs/graphics/math-extra.c` | `obs-graphics::math_extra` | `polar_to_cart` and `cart_to_polar` set `w` to 0. C leaves that lane uninitialized except when `cart_to_polar` zeros a length within `EPSILON`, so parity compares xyz only. `polar_to_norm` does not write `w`. `vec3_close` is one-sided, so `calc_torque` copies `v2` when every lane of `v1 - v2` is `< EPSILON`. `dst == v1` uses the `v1` read before the store. `rand_float` calls the C library `rand` / `RAND_MAX`. Header-inline `get_percentage` and `get_percentagei` stay C. |
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
| `libobs/media-io/audio-io.c` | `obs-media-io::audio_io` | Swapped on Unix and Windows in `libobs-with-rust`. A null input callback is treated as a failed tick; C calls it. A null output callback connects and is skipped at dispatch; C faults on the next dispatch. A removal that shrinks the input list below the loop index is skipped; C reads the stale darray tail. The stop flag is atomic; C uses `os_event_try`. The worker is a Rust thread. Windows locks with a `CRITICAL_SECTION` and registers the MMCSS "Audio" task. The resampler and FFmpeg wrapper stay C. |
| `libobs/util/util_uint64.h` (header-inline, not swapped) | `obs-util::util_uint64` | No intentional differences. Like the header, `mul_div64` does exact 128-bit math on MSVC x64 and `(num / div) * mul + (rem * mul) / div` in wrapping u64 elsewhere, so the two differ once `rem * mul` overflows. Panics where the C is UB or faults: `div == 0`, and on MSVC x64 a quotient past 64 bits; parity excludes those. |
| `libobs/media-io/audio-math.h` (header-inline, not swapped) | `obs-media-io::audio_math` | No intentional differences. `mul_to_db` and `db_to_mul` call the platform `log10f`/`powf` like the header, so parity is exact bit equality on each platform. |
| `libobs/media-io/video-frame.c` (+ `video-frame.h` inlines, not swapped) | `obs-media-io::video_frame` | No intentional differences. Only `video_frame_init` and `video_frame_copy` are swapped; `video_frame_get_linesizes` / `_get_plane_heights` have no C symbol. V210 keeps the C's `ALIGN` to 48, which only rounds to a power of two (width 1280 gives linesize 3424, not 3456). The `u32` math wraps as in C. `VIDEO_FORMAT_NONE`, an unknown format and a zero width or height still reach `bmalloc(0)`, which crashes. `enum video_format` crosses the C ABI as a `c_int`. |
| `libobs/media-io/format-conversion.c` | `obs-media-io::format_conversion` | No intentional differences. The C's SSE2 is integer only, so the port is scalar and parity is byte-exact. The planes are `Source`/`Sink` traits: byte slices panic where the C reads or writes out of bounds, and the shims read the caller's raw pointers at the C's offsets. Quirks kept: `decompress_422` reads and writes twice each row's width, so rows spill into the next. |
| `libobs/util/platform-windows.c` | `obs-util::platform::windows::host` | Swapped on Windows in `libobs-with-rust`. `os_dir`, `os_cpu_usage_info`, and `os_inhibit_t` are `Box` values, freed by the close and destroy shims. Returned path strings are `bmalloc`. A null `os_glob` out-pointer returns -1 and writes nothing. Null `os_dlsym`, `os_dlclose`, `os_readdir`, and `os_closedir` do not crash. Version, core-count, and total-memory caches take a mutex; the stored values match the C statics. `DllMain` sets the timer period only. libobs does not define `PTW32_STATIC_LIB`, so the pthread static-lib hooks stay out. A short plugin image is caught with MSVC `__try` in `plugin_seh.c`; MinGW runs the same checks without that handler. `get_win_release_id` and `DllMain` stay hidden. |
| `libobs/util/platform-nix.c` | `obs-util::platform::unix::nix` | Swapped on macOS, Linux, and FreeBSD. The Apple clock, paths, CPU, memory, and inhibit live in the Cocoa platform module. `os_inhibit_sleep_*` on Linux and FreeBSD is `util/platform-nix-inhibit.c`, the next row. `os_opendir` copies the path (C keeps the caller's pointer). Null pointers that would crash return an error or null. Glob paths are copies freed by `os_globfree`. `os_dir` and `os_cpu_usage_info` are `Box` values, not `bmalloc`. Core-count and total-memory caches take a mutex. A missing `$HOME` still `bcrash`es in the shim. FreeBSD total memory reads `hw.physmem`. |
| `libobs/util/platform-nix-inhibit.c.h` | `obs-util::platform::unix::nix_inhibit` | Swapped on Linux and FreeBSD in `libobs-with-rust`. The portal and session D-Bus inhibitors stay C (`platform-nix-portal.c`, `platform-nix-dbus.c`) and are called as before: the portal first, the session inhibitors only without one, nothing in a Flatpak without the portal, else the `xdg-screensaver reset` thread every 30 s. `GIO_FOUND` is the `libobs-rust` feature `gio`, set by CMake from `find_package(Gio)` (on by default for a plain Cargo build). The inhibitor is a `Box` and the reason a `CString` (C: `bzalloc`, `bstrdup`). A thread that cannot start returns false and stays inactive; C checks `pthread_create < 0`, never true, and reports success. The reset retries its wait only when interrupted; C retries any `waitpid` error, so with `SIGCHLD` ignored it never stops. The child is started by std's `Command`, with the C's empty signal mask and default `SIGPIPE`. |
| `libobs/util/platform-cocoa.m` | `obs-util::platform::unix::cocoa` | Swapped on macOS in `libobs-with-rust`. `os_cpu_usage_info` and `os_inhibit_t` are `Box` values. Core-count and total-memory caches take a mutex. A null `os_get_proc_memory_usage` returns false. A null `cfstr_copy_dstr` dstr pointer returns false. The `cfstr_copy_dstr` slow path zeros the gap past the first NUL, and the `cfstr_copy_cstr` slow path is sized to the C string. An empty inhibit reason is `CFStringCreateWithCString("")`. A null `UTF8String` becomes an empty base path. |
| `libobs/util/platform-time.c.h` (extracted from `platform.c`) | `obs-util::platform_time` | Swapped on every OS in `libobs-with-rust`. The base pair is recorded by the first call, a NULL `storage` included, as the C's `pthread_once` does, not when the library loads. The wall clock is `SystemTime::now`, which cannot fail, so the C's NULL return when `timespec_get` fails does not happen. Keeps the C carry above the base: a result on a whole second is `tv_nsec == 1000000000` with `tv_sec` one short (characterized, not endorsed; #1505). `struct timespec` keeps the host `time_t` and a C `long` `tv_nsec` (32-bit on Windows). |
| `libobs/util/platform-filename.c.h` (extracted from `platform.c`) | `obs-util::platform_filename` | Swapped on every OS in `libobs-with-rust`. As in C, each call reads `time()`, then `localtime()` (not `localtime_r`: it re-reads `TZ` and shares the C library's buffer; the thread-safety question on #262 is pinned, not fixed), then libobs's `obs_get_video_info`, once and before the format is looked at, and the date specifiers and `%FPS`'s `%.2f` come from the host's `strftime` and `snprintf`, so names, offsets and rounding are the host C library's. Kept, characterized, not endorsed: `%FPS` keeps two decimals for a whole rate when `fps_den > 1` (`50/2` is `25.00`), and the result of `obs_get_video_info` is not checked (#1509). The C then reads the fields the call did not write from its uninitialized `struct obs_video_info`; Rust cannot read uninitialized memory, so its struct starts zeroed and gives `0`, `0x0` and `None` there. Before `obs_startup` both crash in `obs_get_video_info`. A `localtime` that fails (not for the current time) gives empty date conversions; C passes NULL to `strftime`. `get_video_format_name` is a table in the core, since `obs-media-io` depends on `obs-util`. Tier 2's `test_formatted_filename` compiles the C into itself, so the Tier 3 parity test is what covers the swap. |
| `libobs/obs-data.c` | `obs-data` | Swapped in `libobs-with-rust`. Objects, items and arrays are `Arc<RwLock<_>>`, so they can be shared across threads like the C objects (atomic reference count, concurrent reads); a set of a new name is one step under the write lock, so two threads cannot add the same name twice (C can). JSON text comes from jansson 2.14, same flags as the C file. Item pointers stay stable across set; C may realloc the item. A user value left in place when `set_autoselect` changes the type is not read back as the new type; those getters return the empty value. The safe core replaces a file by removing the destination and renaming, and it does not log; the C shim calls `os_quick_write_utf8_file` and `os_quick_write_utf8_file_safe`. A null name, or a null vec out-pointer, does nothing. `obs_data_newref` stays inline in the header. |
| `libobs/callback/calldata.c.h` | `obs-callback` | Swapped in `libobs-with-rust`. Only `calldata_get_data`, `calldata_set_data`, and `calldata_get_string` move. The header inlines stay, including create, destroy, clear, and free. A non-fixed stack is still `bmalloc` / `brealloc` / `bfree`. A null `out`, a null string out-pointer, or a null stack with a non-zero size returns false; C would read or write through the null. A walk past the used size is not found. Well-formed stacks match, including the exact-fit grow. |
| `libobs/callback/decl.c.h` | `obs-callback::decl` | Swapped in `libobs-with-rust`. `parse_decl_string` moves; `decl_param_free` and `decl_info_free` stay inline in the header. Names are `bmalloc` copies and the parameters a `bmalloc` darray. Tokens come from `obs-util::cf_tokenizer`; the parse steps are the `cf-parser.h` inlines decl.c calls, so errors, positions, and the logged warning match. `cf-lexer.c` stays C (group `libobs.util-cf`); the two `cf-parser.c` exports are in `obs-util::cf_parser`. A `#` first on a line is an error (C runs the preprocessor; `#include` reads a file). A first token that is not a name, or a function name that is not a name, fails with the C errors; C reads an uninitialized return type or calls `strcmp` on null there. A null `decl` returns false. |
| `libobs/callback/proc.c.h` | `obs-callback` | Swapped in `libobs-with-rust`. The four `proc_handler_*` exports move. The handler is a `Box`. Declaration strings stay `bmalloc` / `bfree`, and `bnum_allocs` still balances across create and destroy. `parse_decl_string` is the `obs-callback::decl` port. A null handler returns without allocating. A null procedure name on an empty handler returns false. Once a proc is registered, the shim faults in `strcmp`. A null callback is stored and faults if called. The duplicate-name warning is logged after the table lock is released. `pthread_mutex_init_recursive` failure is not reproduced. |
| `libobs/obs-missing-files.c.h` | `obs-missing-files` | Swapped in `libobs-with-rust`. The list, each file, the path, and a source name are `bmalloc` / `bstrdup` / `bfree`, and the pointer array is the `bmalloc` darray. `add_file` does not take a reference; `append` does. `destroy` frees a file while its ref is still 1. A script or any other type stores the callback and does not call it. Append of a list to itself keeps walking as the array grows. `obs_source_get_name` and `obs_source_replace_missing_file` stay in `obs-source.c.h`. No intentional differences. |
| `libobs/obs-scene-blend.c.h` | `obs-scene-blend` | Swapped in `libobs-with-rust`. The file is one table, so the shim is the table: `obs_blend_mode_params`, a `#[no_mangle]` static built at compile time from the safe core, with the row struct `struct obs_blend_mode_param` from the private `libobs/obs-scene-blend.h`. The table now has external linkage in C as well (it was `static`), so `obs-scene.c` reads it from another translation unit; the symbol is hidden in both libobs builds. `render_item_texture()` still indexes it unchecked: a mode past `OBS_BLEND_DARKEN` reads past the end in both. No intentional differences. |
| `libobs/obs-scene-bounds.c.h` | `obs-scene-bounds` | Swapped in `libobs-with-rust`. `obs-scene.c` calls `obs_bounds_fit_scale` inside libobs. The C file has no `EXPORT`, so the shim stays hidden. None and any other type leave the scale unchanged. A zero size on the axis a fit divides by leaves the scale unchanged. `vec2_mulf` stays the header inline. No intentional differences. |
| `libobs/audio-monitoring/pulse/pulseaudio-stream-flags.c.h` | `obs-audio-monitoring::pulse_stream_flags` | Swapped in `libobs-with-rust` on Linux and FreeBSD with `ENABLE_PULSEAUDIO` (group `libobs.ported-pulse`); `pulseaudio-output.c` calls the three functions inside libobs. The C file has no `EXPORT`, so the shims stay hidden. They are built on every host and unused where PulseAudio monitoring is not. The `<pulse/def.h>` values are Rust constants, compared against libpulse's header by the parity test where it is installed (the oracle's stand-in header defines them elsewhere). The `uint64_t` arithmetic wraps as in C. A null `backlog` returns 0 (C dereferences it). `test_pulse_monitor_flags` and `test_pulse_monitor_backlog` compile the C into themselves, so only Tier 3 compares the swap. |
| `libobs/obs-nix-hotkey-state.c.h` | `obs-hotkey::nix_hotkey_state` | Swapped in `libobs-with-rust` on Linux and FreeBSD with `ENABLE_WAYLAND` (group `libobs.ported-nix-hotkey-state`); `obs-nix-wayland.c` and `obs-nix-portal-shortcuts.c` call the seven functions inside libobs. The C file has no `EXPORT`, so the shims stay hidden. They are built on every host and unused where Wayland is not. The lock is `obs->hotkeys.mutex`, taken through libobs's hidden `obs_hotkeys_mutex_lock` / `obs_hotkeys_mutex_unlock` at the same points as the C (none for an out-of-range key or for `free`); the shim never reads `obs`. The shortcut list stays the caller's `bmalloc` darray, grown and erased as `da_push_back` and `da_erase` do. A null `state` returns (false) without locking (C dereferences it). Tier 3 compares return values, lock calls, keys, modifiers and the darray (count, capacity, items) after every step of random event sequences. `test_wayland_hotkeys`, `test_portal_shortcuts` and `test_wayland_multiseat` compile the C into themselves, so only Tier 3 compares the swap. |
| `libobs/obs-nix-portal-shortcut-ids.c.h` | `obs-hotkey::portal_shortcut_ids` | Swapped in `libobs-with-rust` on Linux and FreeBSD with `ENABLE_WAYLAND` and GIO (group `libobs.ported-nix-portal-shortcut-ids`, `LIBOBS_WAYLAND_GIO_PORTED_SOURCES`); `obs-nix-portal-shortcuts.c` calls `obs_nix_portal_shortcut_id`, `_combo` and `_trigger` inside libobs. The C file has no `EXPORT`, so the shims stay hidden. They call nothing from GIO, so they are built on every host, with no Cargo feature, and unused where the portal client is not. `bindable` stays the `static inline` in `obs-nix-portal-shortcuts.h`; the core has the same check. Ids and triggers are written to the caller's `dstr` with the C's calls in the C's order (`dstr_free`, one `dstr_cat` per piece, `dstr_cat_dstr` for the key name), so bytes, length and capacity match. Kept, characterized, not endorsed: modifier bits other than ctrl, alt, shift and super are dropped, and the parser takes modifiers in any order and repeated. A null `id` or `trigger` dstr, a null id string or a null `combo` out-pointer returns false, and a null `trigger` does not call `key_name` (C dereferences them). Tier 3 compares `bindable`, the key codes, the modifier table, and random combinations, id strings, key name callbacks and starting `dstr`s: the result, the output bytes, length and capacity, the out-value and the callback calls. `test_portal_shortcut_ids` and `test_portal_shortcuts` compile the C into themselves, so only Tier 3 compares the swap. |

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
