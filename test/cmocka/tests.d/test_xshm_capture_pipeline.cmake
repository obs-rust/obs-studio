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
    PRIVATE
      OBS::libobs
      OBS::glad
      X11::X11
      X11::XCB
      XCB::XCB
      XCB::XFIXES
      XCB::RANDR
      XCB::SHM
      XCB::XINERAMA
      XCB::COMPOSITE
      ${CMOCKA_LIBRARIES}
  )
  add_dependencies(cmocka-tests test_xshm_capture_pipeline)
  add_test(NAME test_xshm_capture_pipeline COMMAND test_xshm_capture_pipeline)
  set_tests_properties(test_xshm_capture_pipeline PROPERTIES TIMEOUT 10)

  if(TARGET OBS::libobs-opengl)
    add_executable(test_xshm_capture_live test_xshm_capture_live.c)
    target_include_directories(
      test_xshm_capture_live
      PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/plugins/linux-capture"
    )
    target_link_libraries(
      test_xshm_capture_live
      PRIVATE
        OBS::libobs
        OBS::glad
        X11::X11
        X11::XCB
        XCB::XCB
        XCB::XFIXES
        XCB::RANDR
        XCB::SHM
        XCB::XINERAMA
        XCB::COMPOSITE
        ${CMOCKA_LIBRARIES}
    )
    target_compile_definitions(test_xshm_capture_live PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
    add_dependencies(test_xshm_capture_live libobs-opengl)
    add_dependencies(cmocka-tests test_xshm_capture_live)
    find_program(OBS_TEST_XVFB_RUN xvfb-run)
    if(OBS_TEST_XVFB_RUN)
      add_test(
        NAME test_xshm_capture_live
        COMMAND
          "${OBS_TEST_XVFB_RUN}" -a "--server-args=-screen 0 1280x1024x24 -noreset"
          $<TARGET_FILE:test_xshm_capture_live>
        WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
      )
      set_tests_properties(test_xshm_capture_live PROPERTIES TIMEOUT 30 ENVIRONMENT "LP_NUM_THREADS=1")
    endif()
  endif()
endif()
