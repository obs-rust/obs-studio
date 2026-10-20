# linux-capture: the XSHM screen capture follows a change of the X screen's
# resolution (upstream obsproject/obs-studio PR #12937). Builds the
# linux-capture unity file directly, as test_xcomposite_window_events does,
# and resizes the Xvfb screen ctest runs under through RandR, so it runs
# alone. Skips without a display.
if(OS_LINUX AND TARGET linux-capture AND TARGET OBS::libobs-opengl)
  find_package(X11 REQUIRED)
  find_package(X11_XCB REQUIRED)
  find_package(
    XCB
    REQUIRED XCB XFIXES RANDR SHM XINERAMA COMPOSITE
  )
  add_executable(
    test_xshm_resolution_change
    test_xshm_resolution_change.c
    "${CMAKE_SOURCE_DIR}/plugins/linux-capture/build/linux-capture.nix+.h.c"
  )
  target_include_directories(
    test_xshm_resolution_change
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/plugins/linux-capture"
  )
  target_link_libraries(
    test_xshm_resolution_change
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
  # A build tree has no libobs-opengl on the loader path, so load it by path.
  target_compile_definitions(
    test_xshm_resolution_change
    PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
  )
  add_dependencies(test_xshm_resolution_change libobs-opengl)
  add_dependencies(cmocka-tests test_xshm_resolution_change)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_xshm_resolution_change
    COMMAND test_xshm_resolution_change
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_xshm_resolution_change PROPERTIES TIMEOUT 60 RUN_SERIAL TRUE)
endif()
