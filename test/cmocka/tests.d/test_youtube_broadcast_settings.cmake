# The YouTube broadcast settings that "Remember these settings" keeps in
# the profile. Builds the plain C helper the YouTube broadcast dialog calls,
# with an in-memory config, so the test needs neither Qt, a display, nor the
# YouTube integration (which builds only with OAuth keys and browser panels)
# and runs everywhere.
add_executable(
  test_youtube_broadcast_settings
  test_youtube_broadcast_settings.c
  "${CMAKE_SOURCE_DIR}/frontend/utility/youtube-broadcast-settings.c.h"
)
target_include_directories(
  test_youtube_broadcast_settings
  PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility"
)
target_link_libraries(test_youtube_broadcast_settings PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_youtube_broadcast_settings)
add_test(NAME test_youtube_broadcast_settings COMMAND test_youtube_broadcast_settings)
