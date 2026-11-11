# Actual-library source-type theme icon characterization in both libobs modes.
add_executable(test_source_theme_icon test_source_theme_icon.c)
target_link_libraries(test_source_theme_icon PRIVATE OBS::libobs OBS::native-test-cmocka)
add_dependencies(cmocka-tests test_source_theme_icon)
if(XVFB_RUN)
  add_test(
    NAME test_source_theme_icon
    COMMAND ${XVFB_RUN} -a -s "-screen 0 1280x1024x24 -noreset" $<TARGET_FILE:test_source_theme_icon>
  )
else()
  add_test(NAME test_source_theme_icon COMMAND test_source_theme_icon)
endif()
set_tests_properties(test_source_theme_icon PROPERTIES TIMEOUT 30)
