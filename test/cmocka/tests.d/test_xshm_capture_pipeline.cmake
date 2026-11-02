# The real capture tick, with deterministic X transport and upload boundaries.
if(OS_LINUX AND TARGET linux-capture)
  add_executable(
    test_xshm_capture_pipeline
    test_xshm_capture_pipeline.c
    "${CMAKE_SOURCE_DIR}/plugins/linux-capture/xcursor-xcb.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/linux-capture/xhelpers.c.h"
  )
  target_include_directories(
    test_xshm_capture_pipeline
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/plugins/linux-capture"
  )
  target_link_libraries(
    test_xshm_capture_pipeline
    PRIVATE OBS::libobs OBS::glad X11::X11 X11::XCB XCB::XCB XCB::XFIXES XCB::RANDR XCB::SHM XCB::XINERAMA
            XCB::COMPOSITE ${CMOCKA_LIBRARIES}
  )
  add_dependencies(cmocka-tests test_xshm_capture_pipeline)
  add_test(NAME test_xshm_capture_pipeline COMMAND test_xshm_capture_pipeline)
  set_tests_properties(test_xshm_capture_pipeline PROPERTIES TIMEOUT 10)
endif()
