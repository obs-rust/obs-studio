# Read and poll text files the way the FreeType 2 text source does.
if(TARGET OBS::text-freetype2)
  add_executable(test_text_file test_text_file.c "${CMAKE_SOURCE_DIR}/plugins/text-freetype2/text-file.c")
  target_include_directories(test_text_file PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/text-freetype2")
  target_link_libraries(test_text_file PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_text_file)
  add_test(test_text_file ${CMAKE_CURRENT_BINARY_DIR}/test_text_file)
endif()
