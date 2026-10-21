# Characterization of the hotkey binding codec in
# libobs/obs-hotkey-bindings.c before its port (#1621). Unlike
# test_hotkey_load, it links the real libobs and goes through the public
# hotkey API only, so it also covers the Rust lane.
add_executable(test_hotkey_bindings_codec test_hotkey_bindings_codec.c)
target_include_directories(test_hotkey_bindings_codec PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_hotkey_bindings_codec PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_hotkey_bindings_codec)
# obs_startup needs an X display on Linux.
find_program(XVFB_RUN xvfb-run)
if(XVFB_RUN)
  add_test(test_hotkey_bindings_codec ${XVFB_RUN} -a ${CMAKE_CURRENT_BINARY_DIR}/test_hotkey_bindings_codec)
else()
  add_test(test_hotkey_bindings_codec ${CMAKE_CURRENT_BINARY_DIR}/test_hotkey_bindings_codec)
endif()
