# obs-websocket's GetSourceScreenshot/SaveSourceScreenshot must encode a
# high-bit-depth source (P010, I010, I210: linear float in libobs) to sRGB, so
# the screenshot shows the colours of the program output instead of a darker
# picture. Builds the plugin's screenshot helper straight from its sources
# against real libobs and the built OpenGL module, with Qt Gui for QImage, so
# it runs where the plugin itself is off (Tier 2). Needs core video, so it
# runs (under xvfb-run) on Linux and skips without a display.
set(_ws_src "${CMAKE_SOURCE_DIR}/plugins/obs-websocket/src")
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  if(NOT TARGET Qt6::Gui)
    find_package(Qt6 QUIET COMPONENTS Gui)
  endif()
  if(TARGET Qt6::Gui AND EXISTS "${_ws_src}/utils/SourceScreenshot.cpp")
    add_executable(test_ws_screenshot_srgb test_ws_screenshot_srgb.cpp "${_ws_src}/utils/SourceScreenshot.cpp")
    target_compile_definitions(test_ws_screenshot_srgb PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
    target_include_directories(test_ws_screenshot_srgb PRIVATE ${CMOCKA_INCLUDE_DIR} "${_ws_src}/utils")
    target_compile_features(test_ws_screenshot_srgb PRIVATE cxx_std_17)
    target_link_libraries(test_ws_screenshot_srgb PRIVATE OBS::libobs Qt6::Gui ${CMOCKA_LIBRARIES})
    add_dependencies(test_ws_screenshot_srgb libobs-opengl)
    add_dependencies(cmocka-tests test_ws_screenshot_srgb)
    # libobs finds its effect files relative to the working directory.
    add_test(
      NAME test_ws_screenshot_srgb
      COMMAND test_ws_screenshot_srgb
      WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
    )
  endif()
endif()
unset(_ws_src)
