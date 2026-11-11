# The distribution, desktop and session lines of the Linux startup log
# through the real libobs (obs_startup() runs log_system_info()), so the
# hybrid build checks the Rust log rules (rust/obs-core, nix_system_info)
# through the callers in libobs/obs-nix.c and obs-nix-os-release.h, and
# libobs-c-reference checks the C. The expected distribution comes from
# read_distribution_info() over the same /etc/os-release; that header calls
# the hidden rules of obs-nix-log-rules.c, so the test builds that C file
# into itself as the expectation.
# Registered once in build/test-graph.d, consumed by CMake and Cargo.
