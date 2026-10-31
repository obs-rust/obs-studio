# Rust shims hidden from libobs, one fragment per FFI module

`<crate>-<ffi module>.txt` lists the `#[unsafe(no_mangle)]` shims in
`rust/<crate>/src/ffi/<ffi module>.rs` that the C build does not `EXPORT`, one
plain symbol per line; `#` starts a comment. Exported shims go in
`../rust-exports.d/` instead; Rust runtime patterns, linker-specific lines and
C glue go in `../rust-hidden-base.txt`.

`../rust-hidden.cmake` generates the ELF version script (`local:`) and the
ld64 unexports list (`_name`) from these files at configure time, and
`rust/obs-build` does the same, so a port adds its own fragment rather than
editing a shared list (obs-rust/obs-studio#3233). Windows reads none of them,
so `ci/ci_scope.py` runs a fragment-only change on Linux.
`rust/obs-util/tests/platform/export_list.rs` checks that every name is a shim
of the fragment's module and that every shim is exported or hidden;
`ci/test_hidden_symbol_fragments.py` pins the layout and the generator.
