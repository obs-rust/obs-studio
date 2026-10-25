# plugins/obs-qsv11: a live bitrate change resets the encoder under the
# lock every QSV session shares (#2291). Compiles
# plugins/obs-qsv11/obs-qsv11.c.h with fake qsv_encoder_* functions; needs
# the VPL headers only, no Intel GPU.
if(OS_LINUX)
  find_package(VPL 2.9 QUIET)
endif()
if(OS_LINUX AND TARGET VPL::VPL)
  add_executable(test_qsv_update test_qsv_update.c)
  target_include_directories(
    test_qsv_update
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" $<TARGET_PROPERTY:VPL::VPL,INTERFACE_INCLUDE_DIRECTORIES>
  )
  target_compile_definitions(test_qsv_update PRIVATE OBS_QSV_HOST_DEFS="plugins/obs-qsv11/bits/linux_defs.h")
  # As the plugin itself builds obs-qsv11.c.h.
  target_compile_options(test_qsv_update PRIVATE $<$<C_COMPILER_ID:GNU,Clang>:-Wno-error=switch>)
  target_link_libraries(test_qsv_update PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_qsv_update)
  add_test(test_qsv_update ${CMAKE_CURRENT_BINARY_DIR}/test_qsv_update)
  set_tests_properties(test_qsv_update PROPERTIES TIMEOUT 60)
endif()
