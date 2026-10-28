# Characterization of the crash report rules of
# libobs/obs-win-crash-handler.c, moved to libobs/obs-win-crash-report.c
# before their port to rust/obs-core (win_crash_report): which module holds
# an address, the thread passes, the frame line, a 32-bit build's masks, the
# thread name prefix and when the filter passes the exception on. Builds the
# moved file into itself (it takes no Windows type), so it runs on every
# platform. The functions are hidden in libobs, so this characterizes that C
# source; the test calls only them. Target and test are declared in
# ../build/test-graph.json.
