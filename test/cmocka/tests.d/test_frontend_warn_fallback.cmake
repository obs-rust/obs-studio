# obs_frontend_warn_fallback() without a frontend (obs-rust/obs-studio#2218):
# the API library logs at error level and accepts at once. test/cmocka is
# configured before frontend/, so OBS::frontend-api resolves when the build
# is generated; Linux, and skipped when the frontend is not built.
if(OS_LINUX AND ENABLE_FRONTEND)
  add_executable(test_frontend_warn_fallback test_frontend_warn_fallback.c)
  target_include_directories(test_frontend_warn_fallback PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_frontend_warn_fallback PRIVATE OBS::libobs OBS::frontend-api ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_frontend_warn_fallback)
  add_test(test_frontend_warn_fallback ${CMAKE_CURRENT_BINARY_DIR}/test_frontend_warn_fallback)
endif()
