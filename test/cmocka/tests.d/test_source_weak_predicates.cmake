# Characterize both predicates through the selected real libobs on every host.
add_executable(test_source_weak_predicates test_source_weak_predicates.c)
target_include_directories(test_source_weak_predicates PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_source_weak_predicates PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_source_weak_predicates)
if(XVFB_RUN)
  add_test(
    NAME test_source_weak_predicates
    COMMAND ${XVFB_RUN} -a -s "-screen 0 1280x1024x24 -noreset" $<TARGET_FILE:test_source_weak_predicates>
  )
else()
  add_test(NAME test_source_weak_predicates COMMAND test_source_weak_predicates)
endif()
set_tests_properties(test_source_weak_predicates PROPERTIES TIMEOUT 30)
