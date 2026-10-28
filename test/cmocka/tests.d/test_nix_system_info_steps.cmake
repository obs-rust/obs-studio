# Characterization of the system info parsing of libobs/obs-nix.c, moved to
# libobs/obs-nix-system-info.c before its port to rust/obs-core
# (nix_system_info): the /proc/cpuinfo key prefixes, values, physical ids and
# print rule, saved x86 and arm64 cpuinfo text run through a copy of
# log_processor_info()'s loop, the FreeBSD dmesg.boot speed, and the memory
# sizes of sysinfo() and OpenBSD's HW_PHYSMEM64. Builds the moved file into
# itself, with libobs only for the dstr calls, so it runs on every platform.
# The functions are hidden in libobs, so this characterizes that C source; the
# test calls only them. Target and test are declared in
# ../build/test-graph.json.
