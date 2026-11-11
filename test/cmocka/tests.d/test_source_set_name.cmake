# Actual-library characterization shared by the C reference and hybrid lanes.
add_executable(test_source_set_name test_source_set_name.c)
target_include_directories(test_source_set_name PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_source_set_name PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_source_set_name)
if(OS_LINUX AND OBS_XVFB_RUN_REAL)
  add_test(
    NAME test_source_set_name
    COMMAND ${OBS_XVFB_RUN_REAL} -a -s "-screen 0 1280x720x24 -noreset" $<TARGET_FILE:test_source_set_name>
  )
else()
  add_test(NAME test_source_set_name COMMAND test_source_set_name)
endif()
set_tests_properties(test_source_set_name PROPERTIES TIMEOUT 30)
