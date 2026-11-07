# Compile the real URL mapper, including its normal owning PCH. No GPU,
# display, live ingest or service account is needed.
if(TARGET OBS::file-updater AND TARGET jansson::jansson)
  add_executable(test_rtmps_nanostream test_rtmps_nanostream.c)
  target_include_directories(test_rtmps_nanostream PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}")
  target_link_libraries(
    test_rtmps_nanostream
    PRIVATE OBS::libobs OBS::file-updater jansson::jansson ${CMOCKA_LIBRARIES}
  )
  add_dependencies(cmocka-tests test_rtmps_nanostream)
  add_test(NAME test_rtmps_nanostream COMMAND test_rtmps_nanostream)
  set_tests_properties(test_rtmps_nanostream PROPERTIES TIMEOUT 10)
endif()
