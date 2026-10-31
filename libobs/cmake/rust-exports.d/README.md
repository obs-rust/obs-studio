# Rust C ABI exports, one fragment per FFI module

`<crate>-<ffi module>.txt` lists every exported `#[unsafe(no_mangle)]` shim in
`rust/<crate>/src/ffi/<ffi module>.rs`, one symbol per line, sorted. A trailing
` unix` marks a `cfg(unix)` shim: Windows keeps the C version (e.g.
`util/pipe-windows.c`), so `libobs/cmake/rust.cmake` does not `/EXPORT` it
there. Symbols hidden from libobs's ABI go in `../rust-hidden.d/<crate>-<ffi module>.txt`
instead.

A port adds its own fragment rather than editing a shared list, so concurrent
port PRs do not conflict (obs-rust/obs-studio#137).
`rust/obs-util/tests/export_list.rs` enforces one fragment per module with
exactly that module's shims.
