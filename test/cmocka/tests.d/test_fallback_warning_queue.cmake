# Fallback warning glue (obs-rust/obs-studio#2218): frontend/utility/
# FallbackWarning logs at once from any thread, waits for the main window,
# shows one OBSFallbackWarning at a time and answers each caller once on the
# UI thread. Builds the glue and the window (their own unity groups) without
# the frontend, on Qt's offscreen platform, with the frontend's Rust
# staticlib when frontend-rust.cmake builds it (this directory is configured
# before the frontend, so it is a generator expression). Without it the
# glue's C rollback path is tested. blog() is the test's own, so libobs is
# not linked.
if(OS_LINUX)
  find_package(Qt6 QUIET COMPONENTS Widgets Test)
  if(TARGET Qt6::Widgets AND TARGET Qt6::Test)
    set(_fw_dir "${CMAKE_SOURCE_DIR}/frontend")
    add_executable(
      test_fallback_warning_queue
      test_fallback_warning_queue.cpp
      "${_fw_dir}/build/frontend.dialogs-obsfallbackwarning+.hpp.cpp"
      "${_fw_dir}/build/frontend.fallback-warning+.hpp.cpp"
      "${_fw_dir}/dialogs/OBSFallbackWarning.hpp"
      "${_fw_dir}/utility/FallbackWarning.hpp"
    )
    set_target_properties(test_fallback_warning_queue PROPERTIES AUTOMOC ON)
    target_include_directories(
      test_fallback_warning_queue
      PRIVATE
        ${CMOCKA_INCLUDE_DIR}
        "${_fw_dir}"
        "${CMAKE_SOURCE_DIR}"
        $<TARGET_PROPERTY:OBS::libobs,INTERFACE_INCLUDE_DIRECTORIES>
    )
    target_compile_definitions(
      test_fallback_warning_queue
      PRIVATE $<$<TARGET_EXISTS:obs_frontend-static>:OBS_FRONTEND_RUST>
    )
    target_link_libraries(
      test_fallback_warning_queue
      PRIVATE $<TARGET_NAME_IF_EXISTS:obs_frontend-static> Qt6::Widgets Qt6::Test ${CMOCKA_LIBRARIES}
    )
    add_dependencies(cmocka-tests test_fallback_warning_queue)
    obs_test_sanitize(test_fallback_warning_queue address,undefined)
    add_test(test_fallback_warning_queue ${CMAKE_CURRENT_BINARY_DIR}/test_fallback_warning_queue)
    # Qt and fontconfig keep allocations until exit; only memory errors count.
    set_tests_properties(test_fallback_warning_queue PROPERTIES ENVIRONMENT "ASAN_OPTIONS=detect_leaks=0")
    unset(_fw_dir)
  endif()
endif()
