# Characterization of the system info log rules of libobs/obs-windows.c,
# moved to libobs/obs-windows-system-info.c before their port to
# rust/obs-core (win_system_info): the packed version, the memory sizes,
# the gaming feature gate, the second Game Mode read, each feature's line and
# the security product labels. Builds the moved file into itself (it takes
# no Windows type), so it runs on every platform. The functions are hidden in
# libobs, so this characterizes that C source; the test calls only them.
# Target and test are declared in ../build/test-graph.json.
