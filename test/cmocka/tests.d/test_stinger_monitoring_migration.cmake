# The Stinger transition's pre-33.0 audio monitoring migration
# (obs-rust/obs-studio#2638): the old "audio_monitoring" value is converted
# while "enable_monitoring" has no user value. Header-only decision from the
# obs-transitions plugin; needs no display, no libobs and no module.
add_executable(test_stinger_monitoring_migration test_stinger_monitoring_migration.c)
target_include_directories(
  test_stinger_monitoring_migration
  PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/obs-transitions"
)
target_link_libraries(test_stinger_monitoring_migration PRIVATE ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_stinger_monitoring_migration)
add_test(test_stinger_monitoring_migration ${CMAKE_CURRENT_BINARY_DIR}/test_stinger_monitoring_migration)
