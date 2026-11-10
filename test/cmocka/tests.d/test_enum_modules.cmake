# Reuse the loader's Linux module fixture; exercise the actual public libobs API.
if(CMAKE_SYSTEM_NAME STREQUAL "Linux" AND TARGET core_module_stub)
  add_executable(test_enum_modules test_enum_modules.c)
  target_include_directories(test_enum_modules PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_compile_definitions(test_enum_modules PRIVATE CORE_STUB_PATH="$<TARGET_FILE:core_module_stub>")
  target_link_libraries(test_enum_modules PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(test_enum_modules core_module_stub)
  add_dependencies(cmocka-tests test_enum_modules)
  add_test(
    NAME test_enum_modules
    COMMAND xvfb-run -a -s "-screen 0 1280x1024x24 -noreset" $<TARGET_FILE:test_enum_modules>
  )
  set_tests_properties(test_enum_modules PROPERTIES TIMEOUT 30)
endif()
