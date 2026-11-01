# mac-virtualcam: the shared-memory ring that feeds the OBS Virtual Microphone
# HAL plug-in (obs-rust/obs-studio#3184). Plain C11, so it is tested on every
# host.
add_executable(test_mac_vmic_shm test_mac_vmic_shm.c)
target_include_directories(
  test_mac_vmic_shm
  PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/mac-virtualcam/src/common"
)
target_link_libraries(test_mac_vmic_shm PRIVATE ${CMOCKA_LIBRARIES})
if(MSVC)
  # MSVC's <stdatomic.h> errors out ("C atomic support is not enabled")
  # without C11 and its experimental atomics switch.
  set_target_properties(test_mac_vmic_shm PROPERTIES C_STANDARD 11 C_STANDARD_REQUIRED ON)
  target_compile_options(test_mac_vmic_shm PRIVATE /experimental:c11atomics)
endif()
add_dependencies(cmocka-tests test_mac_vmic_shm)
add_test(test_mac_vmic_shm ${CMAKE_CURRENT_BINARY_DIR}/test_mac_vmic_shm)
