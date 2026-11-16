# Direct characterization through the public API, without core startup.
add_executable(test_output_mixer_setters test_output_mixer_setters.c)
target_include_directories(test_output_mixer_setters PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/libobs")
target_link_libraries(
  test_output_mixer_setters
  PRIVATE OBS::libobs ${CMOCKA_LIBRARIES} OBS::caption Uthash::Uthash $<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>
)
add_dependencies(cmocka-tests test_output_mixer_setters)
add_test(NAME test_output_mixer_setters COMMAND test_output_mixer_setters)
