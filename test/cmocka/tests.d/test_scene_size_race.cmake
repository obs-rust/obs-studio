# Scene size getters against scene_load(), for the ThreadSanitizer pass, which
# runs on Linux only (#3569).
if(OS_LINUX)
  add_executable(test_scene_size_race test_scene_size_race.c)
  target_include_directories(test_scene_size_race PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_scene_size_race PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_scene_size_race)
  add_test(NAME test_scene_size_race COMMAND test_scene_size_race)
endif()
