# obs-websocket's GetSourceScreenshot labels its data URL with
# Utils::ImageFormat::MimeType(). Builds that helper straight from the plugin
# sources (no libobs, no Qt, no display), so it runs where the plugin itself
# is off (Tier 2).
set(_ws_src "${CMAKE_SOURCE_DIR}/plugins/obs-websocket/src")
if(EXISTS "${_ws_src}/utils/ImageFormat.cpp")
  add_executable(test_ws_image_mime test_ws_image_mime.cpp "${_ws_src}/utils/ImageFormat.cpp")
  target_include_directories(test_ws_image_mime PRIVATE ${CMOCKA_INCLUDE_DIR} "${_ws_src}")
  target_compile_features(test_ws_image_mime PRIVATE cxx_std_17)
  target_link_libraries(test_ws_image_mime PRIVATE ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_ws_image_mime)
  add_test(test_ws_image_mime ${CMAKE_CURRENT_BINARY_DIR}/test_ws_image_mime)
endif()
unset(_ws_src)
