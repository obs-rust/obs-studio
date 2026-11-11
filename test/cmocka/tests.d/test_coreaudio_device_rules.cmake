# Characterization of the CoreAudio monitoring device rules of
# libobs/audio-monitoring/osx/coreaudio-enum-devices.c, moved to
# libobs/audio-monitoring/osx/coreaudio-device-rules.c and ported to
# rust/obs-audio-monitoring (coreaudio_devices): whether a device is listed,
# the id count, the "default" id, the name match and the virtual device
# check. Builds the moved file on its own (no Apple header) and links libobs
# for astrstri(), so it runs on every platform. The functions are hidden in
# libobs, so this characterizes that C source; the test calls only them. The
# Tier 3 parity tests in rust/obs-audio-monitoring compare the shims with
# this C too.
# Target and test are declared in ../build/test-graph.d.

# The same cases against the Rust shims, built and run only with the hybrid
# backend (OBS_LIBOBS_BACKEND=C has no Rust target), with
# COREAUDIO_DEVICE_RULES_TEST_RUST: the port's intentional difference (the
# count stops at the ids the second call wrote) in place of the C's count.
# coreaudio_device_* are hidden in the shared library, so the test takes them
# from the production staticlib at executable link time and compiles none of
# the C. It includes only coreaudio-device-rules.h, and the
# obs-audio-monitoring member needs nothing from libobs, so it links the
# archive alone. Linux only: the shims have no host code, so one host shows
# the swap.
# Rust-static target and test are declared in ../build/test-graph.d.
