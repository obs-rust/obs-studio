# plugins/obs-qsv11: a wedged device stops the encode with an error instead
# of a silent 0 kbps stall, and a live bitrate change reaches the encoder
# (#2291). Compiles the plugin's real VPL encoder against a scripted fake of
# the libvpl C API; needs the VPL headers only, no Intel GPU.
if(OS_LINUX)
  find_package(VPL 2.9 QUIET)
endif()
if(OS_LINUX AND TARGET VPL::VPL)
  add_executable(test_qsv_encode_bounds test_qsv_encode_bounds.cpp)
  target_include_directories(
    test_qsv_encode_bounds
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" $<TARGET_PROPERTY:VPL::VPL,INTERFACE_INCLUDE_DIRECTORIES>
  )
  target_compile_definitions(test_qsv_encode_bounds PRIVATE OBS_QSV_HOST_DEFS="plugins/obs-qsv11/bits/linux_defs.h")
  # As the plugin itself builds its encoder sources.
  target_compile_options(test_qsv_encode_bounds PRIVATE $<$<CXX_COMPILER_ID:GNU,Clang>:-Wno-error=switch>)
  target_link_libraries(test_qsv_encode_bounds PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_qsv_encode_bounds)
  add_test(test_qsv_encode_bounds ${CMAKE_CURRENT_BINARY_DIR}/test_qsv_encode_bounds)
  set_tests_properties(test_qsv_encode_bounds PROPERTIES TIMEOUT 120)
endif()
