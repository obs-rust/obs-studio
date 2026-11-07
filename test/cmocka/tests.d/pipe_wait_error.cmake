# Registered in ../build/test-graph.json, retaining both original sources and
# real OBS::libobs linkage. Linux-only waitpid wrapping seeds failed-wait status;
# the SIGCHLD/ECHILD case still uses a real child. The wrapper must have default
# visibility: hiding it fails an ASan link when libobs.so references __wrap_waitpid.
