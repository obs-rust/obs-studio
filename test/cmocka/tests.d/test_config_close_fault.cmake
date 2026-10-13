if(NOT UNIX)
  return()
endif()

# Inject final-close errors through libc, without changing libobs.
add_library(config_close_fault SHARED config_close_fault.c)
set_target_properties(
  config_close_fault
  PROPERTIES C_VISIBILITY_PRESET default XCODE_ATTRIBUTE_GCC_SYMBOLS_PRIVATE_EXTERN NO
)
target_link_libraries(config_close_fault PRIVATE ${CMAKE_DL_LIBS})
add_executable(test_config_close_fault test_config_close_fault.c)
target_include_directories(test_config_close_fault PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_config_close_fault PRIVATE config_close_fault OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_config_close_fault)
if(APPLE)
  set(_config_close_preload DYLD_INSERT_LIBRARIES)
else()
  set(_config_close_preload LD_PRELOAD)
endif()
add_test(
  NAME test_config_close_fault
  COMMAND
    ${CMAKE_COMMAND} -E env "${_config_close_preload}=$<TARGET_FILE:config_close_fault>"
    $<TARGET_FILE:test_config_close_fault>
)
set_tests_properties(test_config_close_fault PROPERTIES TIMEOUT 30)
