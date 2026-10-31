# The RTMPS fallback (obs-rust/obs-studio#2205) raises the shared fallback
# warning (obs_frontend_warn_fallback, #2218) with the right id, wording and
# Cancel, passes its answer on, and never falls back when the server's
# certificate did not verify. The warning itself is a fake here; its dialog is
# tested in test_fallback_warning_dialog. Needs Qt Core only.
find_package(Qt6 QUIET COMPONENTS Core)
if(TARGET Qt6::Core AND NOT OS_WINDOWS)
  add_executable(
    test_rtmps_fallback_warning
    test_rtmps_fallback_warning.cpp
    "${CMAKE_SOURCE_DIR}/frontend/utility/RtmpsFallback.hpp"
  )
  target_compile_definitions(
    test_rtmps_fallback_warning
    PRIVATE FRONTEND_LOCALE="${CMAKE_SOURCE_DIR}/frontend/data/locale/en-US.ini"
  )
  target_include_directories(
    test_rtmps_fallback_warning
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/api" "${CMAKE_SOURCE_DIR}/frontend/utility"
  )
  target_link_libraries(test_rtmps_fallback_warning PRIVATE OBS::libobs Qt6::Core ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_rtmps_fallback_warning)
  add_test(test_rtmps_fallback_warning ${CMAKE_CURRENT_BINARY_DIR}/test_rtmps_fallback_warning)
  set_tests_properties(test_rtmps_fallback_warning PROPERTIES TIMEOUT 30)
  cmocka_qt_runtime(test_rtmps_fallback_warning Qt6::Core)
endif()
