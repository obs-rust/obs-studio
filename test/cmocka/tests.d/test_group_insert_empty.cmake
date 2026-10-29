# obs_scene_insert_group() with a non-NULL item list and a count of 0 must add
# an empty group on top, not read before the list (obs-rust/obs-studio#2945).
# Each case forks a child with its own libobs and needs no video;
# obs_startup() needs a display on Linux, so the test runs under xvfb-run when
# it is installed and the cases skip without a display.
if(NOT WIN32)
  add_executable(test_group_insert_empty test_group_insert_empty.c)
  target_include_directories(test_group_insert_empty PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_group_insert_empty PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_group_insert_empty)
  find_program(XVFB_RUN xvfb-run)
  if(OS_LINUX AND XVFB_RUN)
    add_test(NAME test_group_insert_empty COMMAND ${XVFB_RUN} -a $<TARGET_FILE:test_group_insert_empty>)
  else()
    add_test(test_group_insert_empty ${CMAKE_CURRENT_BINARY_DIR}/test_group_insert_empty)
  endif()
endif()
