# Compile the production mapper with its owning PCH; no live service needed.
if(TARGET OBS::file-updater AND TARGET jansson::jansson)
  add_executable(test_rtmps_restream test_rtmps_restream.c)
  target_include_directories(test_rtmps_restream PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}")
  target_link_libraries(test_rtmps_restream PRIVATE OBS::libobs OBS::file-updater jansson::jansson ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_rtmps_restream)
  add_test(NAME test_rtmps_restream COMMAND test_rtmps_restream)
  set_tests_properties(test_rtmps_restream PROPERTIES TIMEOUT 10)
endif()
