# Texture-only encoder on raw video (obsproject/obs-studio#12731).
if(NOT WIN32)
  add_executable(test_texture_encoder_raw_format test_texture_encoder_raw_format.c)
  target_include_directories(test_texture_encoder_raw_format PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_texture_encoder_raw_format PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_texture_encoder_raw_format)
  add_test(test_texture_encoder_raw_format ${CMAKE_CURRENT_BINARY_DIR}/test_texture_encoder_raw_format)
  # The test kills a scenario child after 20 s; this bounds the whole run.
  set_tests_properties(test_texture_encoder_raw_format PROPERTIES TIMEOUT 180)
endif()
