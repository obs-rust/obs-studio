# mac-virtualcam: the OBS Virtual Microphone HAL plug-in's device clock
# (obs-rust/obs-studio#3184). Plain C, so it is tested on every host.
add_executable(test_mac_vmic_clock test_mac_vmic_clock.c)
target_include_directories(
  test_mac_vmic_clock
  PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/mac-virtualcam/src/common"
)
target_link_libraries(test_mac_vmic_clock PRIVATE ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_mac_vmic_clock)
add_test(test_mac_vmic_clock ${CMAKE_CURRENT_BINARY_DIR}/test_mac_vmic_clock)
