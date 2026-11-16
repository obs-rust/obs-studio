# Linux public discovery fixture; no private headers or loader dependencies.
if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
  add_executable(test_find_modules test_find_modules.c)
  target_include_directories(test_find_modules PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_find_modules PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_find_modules)
  add_test(
    NAME test_find_modules
    COMMAND xvfb-run -a -s "-screen 0 1280x1024x24 -noreset" $<TARGET_FILE:test_find_modules>
  )
  set_tests_properties(test_find_modules PROPERTIES TIMEOUT 30)
endif()
