# win-dshow: the Virtual Camera sends a mix track to a virtual audio cable
# (obs-rust/obs-studio#3183). The routing core has no Windows dependencies, so
# it is tested on every host.
add_executable(test_win_vmic_route test_win_vmic_route.c)
target_include_directories(test_win_vmic_route PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/win-dshow")
target_link_libraries(test_win_vmic_route PRIVATE ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_win_vmic_route)
add_test(test_win_vmic_route ${CMAKE_CURRENT_BINARY_DIR}/test_win_vmic_route)
