# obs-ffmpeg: the VA-API encoder and its probe open render nodes
# close-on-exec, so the recording's muxer does not keep a copy (the leak of
# obsproject/obs-studio#14054). Runs the real vaapi-utils.c with
# vaGetDisplayDRM() wrapped; the render node cases skip without one.
if(OS_LINUX AND TARGET OBS::ffmpeg)
  find_package(Libva REQUIRED)
  find_package(Libpci REQUIRED)
  find_package(Libdrm REQUIRED)
  add_executable(
    test_vaapi_open_cloexec
    test_vaapi_open_cloexec.c
    "${CMAKE_SOURCE_DIR}/plugins/obs-ffmpeg/vaapi-probe-policy.c.h"
  )
  target_include_directories(
    test_vaapi_open_cloexec
    PRIVATE
      ${CMOCKA_INCLUDE_DIR}
      "${CMAKE_SOURCE_DIR}"
      "${CMAKE_SOURCE_DIR}/plugins/obs-ffmpeg"
      "${CMAKE_SOURCE_DIR}/shared/opts-parser"
      "$<TARGET_PROPERTY:Libpci::pci,INTERFACE_INCLUDE_DIRECTORIES>"
      "$<TARGET_PROPERTY:Libdrm::Libdrm,INTERFACE_INCLUDE_DIRECTORIES>"
      "$<TARGET_PROPERTY:FFmpeg::avcodec,INTERFACE_INCLUDE_DIRECTORIES>"
  )
  target_link_libraries(
    test_vaapi_open_cloexec
    PRIVATE OBS::libobs Libva::va Libva::drm FFmpeg::avutil ${CMOCKA_LIBRARIES}
  )
  target_link_options(test_vaapi_open_cloexec PRIVATE "LINKER:--wrap=vaGetDisplayDRM")
  # A library's reference to the wrapped function resolves to this
  # executable's __wrap_vaGetDisplayDRM, which must not be hidden.
  set_target_properties(test_vaapi_open_cloexec PROPERTIES C_VISIBILITY_PRESET default)
  add_dependencies(cmocka-tests test_vaapi_open_cloexec)
  add_test(test_vaapi_open_cloexec ${CMAKE_CURRENT_BINARY_DIR}/test_vaapi_open_cloexec)
  set_tests_properties(test_vaapi_open_cloexec PROPERTIES TIMEOUT 60)
endif()
