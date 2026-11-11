# Actual-library API characterization; reuse the loader's existing Linux fixture.
if(CMAKE_SYSTEM_NAME STREQUAL "Linux" AND TARGET core_module_stub)
  add_executable(test_output_module test_output_module.c)
  target_include_directories(test_output_module PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_compile_definitions(test_output_module PRIVATE CORE_STUB_PATH="$<TARGET_FILE:core_module_stub>")
  target_link_libraries(test_output_module PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES})
  if(TARGET OBS::w32-pthreads)
    target_link_libraries(test_output_module PRIVATE OBS::w32-pthreads)
  endif()
  add_dependencies(test_output_module core_module_stub)
  add_dependencies(cmocka-tests test_output_module)
  if(XVFB_RUN)
    add_test(
      NAME test_output_module
      COMMAND ${XVFB_RUN} -a -s "-screen 0 1280x1024x24 -noreset" $<TARGET_FILE:test_output_module>
    )
  else()
    add_test(NAME test_output_module COMMAND test_output_module)
  endif()
  set_tests_properties(test_output_module PROPERTIES TIMEOUT 30)
endif()
