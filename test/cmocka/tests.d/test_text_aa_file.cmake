# Real FreeType file-source callbacks with headless graphics allocations.
# The isolated fixture font is selected through the Linux Fontconfig backend.
if(OS_LINUX AND TARGET OBS::text-freetype2)
  add_executable(
    test_text_aa_file
    test_text_aa_file.c
    "${CMAKE_SOURCE_DIR}/plugins/text-freetype2/text-freetype2.c"
    "${CMAKE_SOURCE_DIR}/plugins/text-freetype2/text-functionality.c"
    "${CMAKE_SOURCE_DIR}/plugins/text-freetype2/text-file.c"
    "${CMAKE_SOURCE_DIR}/plugins/text-freetype2/glyph-atlas.c"
    "${CMAKE_SOURCE_DIR}/plugins/text-freetype2/obs-convenience.c"
    "${CMAKE_SOURCE_DIR}/plugins/text-freetype2/find-font-unix.c"
  )
  target_compile_definitions(test_text_aa_file PRIVATE TEST_FONT_PATH="${CMAKE_CURRENT_SOURCE_DIR}/data/text-aa.ttf")
  target_include_directories(
    test_text_aa_file
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/text-freetype2"
  )
  target_link_libraries(
    test_text_aa_file
    PRIVATE OBS::libobs Freetype::Freetype Fontconfig::Fontconfig ${CMOCKA_LIBRARIES}
  )
  add_dependencies(cmocka-tests test_text_aa_file)
  add_test(test_text_aa_file ${CMAKE_CURRENT_BINARY_DIR}/test_text_aa_file)
endif()
