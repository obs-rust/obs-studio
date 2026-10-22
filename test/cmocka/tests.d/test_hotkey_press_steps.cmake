# The hotkey press state machine (libobs/obs-hotkey-press.c), poll by poll,
# with a scripted key state. Like test_hotkey_load it compiles the C into
# itself with an isolated core, so it characterizes the C only;
# test_hotkey_press_state covers the real libobs.
add_executable(
  test_hotkey_press_steps
  test_hotkey_press_steps.c
  "${CMAKE_SOURCE_DIR}/libobs/obs-hotkey.c.h"
  "${CMAKE_SOURCE_DIR}/libobs/obs-hotkey-bindings.c.h"
  "${CMAKE_SOURCE_DIR}/libobs/obs-hotkey-translations.c.h"
  "${CMAKE_SOURCE_DIR}/libobs/obs-hotkey-press.c.h"
)
target_compile_definitions(test_hotkey_press_steps PRIVATE obs=hotkey_test_obs)
target_include_directories(test_hotkey_press_steps PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/libobs")
target_link_libraries(test_hotkey_press_steps PRIVATE OBS::libobs OBS::caption ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_hotkey_press_steps)
add_test(test_hotkey_press_steps ${CMAKE_CURRENT_BINARY_DIR}/test_hotkey_press_steps)
