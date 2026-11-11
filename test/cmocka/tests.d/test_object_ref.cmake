# Exercise the exported APIs in the selected real libobs, not a source fixture.
add_executable(test_object_ref test_object_ref.c)
target_include_directories(test_object_ref PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_object_ref PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES})
if(TARGET OBS::w32-pthreads)
  target_link_libraries(test_object_ref PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_object_ref)
if(XVFB_RUN)
  add_test(
    NAME test_object_ref
    COMMAND ${XVFB_RUN} -a -s "-screen 0 1280x1024x24 -noreset" $<TARGET_FILE:test_object_ref>
  )
else()
  add_test(NAME test_object_ref COMMAND test_object_ref)
endif()
set_tests_properties(test_object_ref PROPERTIES TIMEOUT 30)
