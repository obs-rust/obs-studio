# The frontend's "Screenshot (Source)" must encode a high-bit-depth source
# (P010, I010, I210: linear float in libobs) to sRGB, so the PNG shows the
# colours of the program output instead of a darker picture. Builds the
# screenshot render helper (frontend/utility/screenshot-render.cpp) against
# real libobs and the built OpenGL module, with Qt Widgets for the headers
# display-helpers.hpp includes, so it runs where the frontend is off (Tier 2).
# Needs core video, so it runs (under xvfb-run) on Linux and skips without a
# display.
set(_fe_util "${CMAKE_SOURCE_DIR}/frontend/utility")
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  if(NOT TARGET Qt6::Widgets)
    find_package(Qt6 QUIET COMPONENTS Widgets)
  endif()
  if(TARGET Qt6::Widgets)
    add_executable(test_frontend_screenshot_srgb test_frontend_screenshot_srgb.cpp "${_fe_util}/screenshot-render.cpp")
    target_compile_definitions(
      test_frontend_screenshot_srgb
      PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
    )
    target_include_directories(test_frontend_screenshot_srgb PRIVATE ${CMOCKA_INCLUDE_DIR} "${_fe_util}")
    target_compile_features(test_frontend_screenshot_srgb PRIVATE cxx_std_17)
    target_link_libraries(test_frontend_screenshot_srgb PRIVATE OBS::libobs Qt6::Widgets ${CMOCKA_LIBRARIES})
    add_dependencies(test_frontend_screenshot_srgb libobs-opengl)
    add_dependencies(cmocka-tests test_frontend_screenshot_srgb)
    # libobs finds its effect files relative to the working directory.
    add_test(
      NAME test_frontend_screenshot_srgb
      COMMAND test_frontend_screenshot_srgb
      WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
    )
  endif()
endif()
unset(_fe_util)
