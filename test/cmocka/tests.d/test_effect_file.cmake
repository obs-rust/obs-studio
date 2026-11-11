# Direct C characterization of the original unit, with deterministic I/O and
# constructor collaborators. The real exported symbol is covered by the unchanged
# test_effect_destroy on both implementations; Rust Tier 3 reuses this fixture.
file(READ "${CMAKE_SOURCE_DIR}/libobs/graphics/graphics_effect_file.c.h" _effect_file_source)
string(REPLACE "#include \"libobs/build/libobs-graphics.pch.h\"" "" _effect_file_source "${_effect_file_source}")
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/graphics_effect_file.inc" "${_effect_file_source}")
add_executable(test_effect_file test_effect_file.c)
target_include_directories(test_effect_file PRIVATE "${CMAKE_CURRENT_BINARY_DIR}")
target_link_libraries(test_effect_file PRIVATE OBS::libobs OBS::native-test-cmocka)
add_test(NAME test_effect_file COMMAND test_effect_file)
add_dependencies(cmocka-tests test_effect_file)
