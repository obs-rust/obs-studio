# Pure plugin-side monitor identity decisions run on every host, without Win32.
add_executable(test_windows_monitor_identity test_windows_monitor_identity.c)
target_include_directories(test_windows_monitor_identity PRIVATE "${CMAKE_SOURCE_DIR}")
add_dependencies(cmocka-tests test_windows_monitor_identity)
add_test(NAME test_windows_monitor_identity COMMAND test_windows_monitor_identity)
