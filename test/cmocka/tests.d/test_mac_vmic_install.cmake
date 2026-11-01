# mac-virtualcam: whether the OBS Virtual Microphone HAL plug-in needs installing
# or updating (obs-rust/obs-studio#3341). Plain C, so it is tested on every host.
add_executable(test_mac_vmic_install test_mac_vmic_install.c)
target_include_directories(
  test_mac_vmic_install
  PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/mac-virtualcam/src/common"
)
target_link_libraries(test_mac_vmic_install PRIVATE ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_mac_vmic_install)
add_test(test_mac_vmic_install ${CMAKE_CURRENT_BINARY_DIR}/test_mac_vmic_install)
