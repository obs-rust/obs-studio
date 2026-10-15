# A pre-31 global.ini without LastVersion must still be migrated to user.ini,
# or the first start shows the auto-configuration wizard
# (obs-rust/obs-studio#815). Builds the Qt-free header OBSApp uses.
add_executable(test_settings_migration test_settings_migration.cpp)
target_include_directories(test_settings_migration PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility")
target_compile_features(test_settings_migration PRIVATE cxx_std_17)
target_link_libraries(test_settings_migration PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_settings_migration)
add_test(test_settings_migration ${CMAKE_CURRENT_BINARY_DIR}/test_settings_migration)
