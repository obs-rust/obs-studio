# These actual-library tests pin the intentional hybrid/reference difference.
# Both Tier 2 modes run the same CTest inventory, so each command selects its
# library explicitly rather than inferring expected behavior from a probe.
if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
  add_executable(test_immediate_texture_units "${CMAKE_CURRENT_LIST_DIR}/../test_immediate_texture_units.c")
  target_include_directories(test_immediate_texture_units PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(
    test_immediate_texture_units
    PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES}
  )
  add_dependencies(cmocka-tests test_immediate_texture_units)
  add_test(
    NAME test_immediate_texture_units_hybrid
    COMMAND
      "${CMAKE_COMMAND}" -E env --modify "LD_LIBRARY_PATH=path_list_prepend:$<TARGET_FILE_DIR:libobs>"
      "$<TARGET_FILE:test_immediate_texture_units>" hybrid
  )
  set_tests_properties(test_immediate_texture_units_hybrid PROPERTIES TIMEOUT 30)
  if(TARGET libobs-c-reference)
    add_dependencies(test_immediate_texture_units libobs-c-reference)
    add_test(
      NAME test_immediate_texture_units_reference
      COMMAND
        "${CMAKE_COMMAND}" -E env --modify "LD_LIBRARY_PATH=path_list_prepend:$<TARGET_FILE_DIR:libobs-c-reference>"
        "$<TARGET_FILE:test_immediate_texture_units>" reference
    )
    set_tests_properties(test_immediate_texture_units_reference PROPERTIES TIMEOUT 30)
  endif()
endif()
