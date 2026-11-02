# Characterization of the debug symbol search path rules of
# libobs/obs-windows.c's reset_win32_symbol_paths(), moved to
# libobs/obs-windows-symbol-paths.c before their port to rust/obs-core
# (win_symbol_paths): the directory of a module's binary, the duplicate check
# and the join. Builds the moved file into itself (it takes no Windows type)
# and links libobs for dstr, so it runs on every platform. The functions are
# hidden in libobs, so this characterizes that C source; the test calls only
# them.
# Target and test are declared in ../build/test-graph.json.
