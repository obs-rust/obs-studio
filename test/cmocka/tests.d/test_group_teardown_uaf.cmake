# obs_shutdown must wait for the whole chain of queued scene/group/source
# destruction (use after free that crashed Windows). Needs no graphics, but
# obs_startup needs an X display on Linux, so run under xvfb-run when it
# exists; the case skips if obs_startup still fails.
add_executable(test_group_teardown_uaf test_group_teardown_uaf.c)
target_include_directories(test_group_teardown_uaf PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_group_teardown_uaf PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_group_teardown_uaf)
find_program(XVFB_RUN_PROGRAM xvfb-run)
if(OS_LINUX AND XVFB_RUN_PROGRAM)
  add_test(NAME test_group_teardown_uaf COMMAND ${XVFB_RUN_PROGRAM} -a $<TARGET_FILE:test_group_teardown_uaf>)
else()
  add_test(NAME test_group_teardown_uaf COMMAND test_group_teardown_uaf)
endif()
