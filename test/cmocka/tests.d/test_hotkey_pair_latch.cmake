# Characterization of the hotkey pair press latch in libobs before its port.
# It links the real libobs and goes through the public API only (a frontend
# pair driven by obs_hotkey_trigger_routed_callback with callback rerouting
# on), so it also covers the hybrid libobs. No key is held and no hotkey
# thread timing is involved, so it builds and runs on every OS.
add_executable(test_hotkey_pair_latch test_hotkey_pair_latch.c)
target_include_directories(test_hotkey_pair_latch PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_hotkey_pair_latch PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_hotkey_pair_latch)
# obs_startup needs an X display on Linux.
find_program(XVFB_RUN xvfb-run)
if(XVFB_RUN)
  add_test(test_hotkey_pair_latch ${XVFB_RUN} -a ${CMAKE_CURRENT_BINARY_DIR}/test_hotkey_pair_latch)
else()
  add_test(test_hotkey_pair_latch ${CMAKE_CURRENT_BINARY_DIR}/test_hotkey_pair_latch)
endif()
