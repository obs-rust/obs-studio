# The hotkey pair press latch (libobs/obs-hotkey-pair.c), one event at a time
# in every latch state. Like test_hotkey_load it compiles the C into itself
# with an isolated core, so it characterizes the C only;
# test_hotkey_pair_latch covers the real libobs.
add_executable(
  test_hotkey_pair_steps
  test_hotkey_pair_steps.c
  "${CMAKE_SOURCE_DIR}/libobs/obs-hotkey.c.h"
  "${CMAKE_SOURCE_DIR}/libobs/obs-hotkey-bindings.c.h"
  "${CMAKE_SOURCE_DIR}/libobs/obs-hotkey-translations.c.h"
  "${CMAKE_SOURCE_DIR}/libobs/obs-hotkey-press.c.h"
  "${CMAKE_SOURCE_DIR}/libobs/obs-hotkey-pair.c.h"
)
target_compile_definitions(test_hotkey_pair_steps PRIVATE obs=hotkey_test_obs)
target_include_directories(test_hotkey_pair_steps PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/libobs")
target_link_libraries(test_hotkey_pair_steps PRIVATE OBS::libobs OBS::caption ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_hotkey_pair_steps)
add_test(test_hotkey_pair_steps ${CMAKE_CURRENT_BINARY_DIR}/test_hotkey_pair_steps)
