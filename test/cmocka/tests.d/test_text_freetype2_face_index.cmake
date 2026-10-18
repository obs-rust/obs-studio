# A font that is not face 0 of its file (a face of a .ttc collection, a named
# instance of a variable font) resolves to its own face index through the Linux
# Fontconfig backend (obsproject/obs-studio#11819).
if(OS_LINUX AND TARGET OBS::text-freetype2)
  add_executable(
    test_text_freetype2_face_index
    test_text_freetype2_face_index.c
    "${CMAKE_SOURCE_DIR}/plugins/text-freetype2/find-font-unix.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/text-freetype2/text-freetype2-host-posix.c.h"
  )
  # Only the collection fixture is visible: no system font directories.
  set(ft2_collection_fonts "${CMAKE_CURRENT_BINARY_DIR}/ft2-collection-fonts")
  file(COPY "${CMAKE_CURRENT_SOURCE_DIR}/data/text-collection.ttc" DESTINATION "${ft2_collection_fonts}")
  file(
    WRITE "${CMAKE_CURRENT_BINARY_DIR}/ft2-collection-fonts.conf"
    "<?xml version=\"1.0\"?>\n<fontconfig><dir>${ft2_collection_fonts}</dir><cachedir>${CMAKE_CURRENT_BINARY_DIR}/ft2-collection-fonts-cache</cachedir></fontconfig>\n"
  )
  target_compile_definitions(
    test_text_freetype2_face_index
    PRIVATE COLLECTION_FONTCONFIG="${CMAKE_CURRENT_BINARY_DIR}/ft2-collection-fonts.conf"
  )
  target_include_directories(
    test_text_freetype2_face_index
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/text-freetype2"
  )
  target_link_libraries(
    test_text_freetype2_face_index
    PRIVATE OBS::libobs Freetype::Freetype Fontconfig::Fontconfig ${CMOCKA_LIBRARIES}
  )
  add_dependencies(cmocka-tests test_text_freetype2_face_index)
  add_test(test_text_freetype2_face_index ${CMAKE_CURRENT_BINARY_DIR}/test_text_freetype2_face_index)
endif()
