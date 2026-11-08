# The standalone Windows-key fixture must not compile the global-reading
# app-ID adapter: it does not own libobs's private core. Cargo.toml keeps
# obs-hotkey's release FFI modules in separate archive members, as in debug,
# so selecting the Windows-key shims does not select the app-ID shim.
# The real adapter stays in libobs's core unity group and the live portal test.
