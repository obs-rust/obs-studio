# Uploading the last crash log with no crash log to upload must report a
# failure; the upload dialog waited for an answer forever
# (obsproject/obs-studio#13401). Builds the real utility/CrashHandler.cpp and
# its Linux part against stand-ins for the frontend app (crash-handler-stubs/):
# QtCore and nlohmann_json, no display, no network.
if(OS_LINUX)
  find_package(Qt6 QUIET COMPONENTS Core)
  find_package(nlohmann_json 3.11 QUIET)
  if(TARGET Qt6::Core AND TARGET nlohmann_json::nlohmann_json)
    add_executable(
      test_crash_log_upload
      test_crash_log_upload.cpp
      "${CMAKE_SOURCE_DIR}/frontend/utility/CrashHandler.cpp.hpp"
      "${CMAKE_SOURCE_DIR}/frontend/utility/CrashHandler.hpp"
      "${CMAKE_SOURCE_DIR}/frontend/utility/CrashHandler_Linux.cpp.hpp"
      "${CMAKE_SOURCE_DIR}/frontend/utility/RemoteTextThread.hpp"
    )
    set_target_properties(test_crash_log_upload PROPERTIES AUTOMOC ON)
    # The stand-ins come first so <OBSApp.hpp> and the frontend's precompiled
    # headers resolve to them.
    target_include_directories(
      test_crash_log_upload
      BEFORE
      PRIVATE
        "${CMAKE_CURRENT_SOURCE_DIR}/crash-handler-stubs"
        ${CMOCKA_INCLUDE_DIR}
        "${CMAKE_SOURCE_DIR}"
        "${CMAKE_SOURCE_DIR}/frontend"
    )
    target_compile_features(test_crash_log_upload PRIVATE cxx_std_17)
    target_link_libraries(
      test_crash_log_upload
      PRIVATE OBS::libobs Qt6::Core nlohmann_json::nlohmann_json ${CMOCKA_LIBRARIES}
    )
    add_dependencies(cmocka-tests test_crash_log_upload)
    add_test(test_crash_log_upload ${CMAKE_CURRENT_BINARY_DIR}/test_crash_log_upload)
    set_tests_properties(test_crash_log_upload PROPERTIES TIMEOUT 60)
  endif()
endif()
