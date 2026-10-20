# Settings > Output, Advanced: a profile that names an encoder this build does
# not have selects a fallback encoder instead of showing "No properties
# available" (obsproject/obs-studio#3531). Builds the plain C helper the
# settings dialog calls, so the test needs neither Qt nor libobs.
add_executable(
  test_encoder_fallback
  test_encoder_fallback.c
  "${CMAKE_SOURCE_DIR}/frontend/utility/encoder-fallback.c.h"
)
target_include_directories(test_encoder_fallback PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility")
target_link_libraries(test_encoder_fallback PRIVATE ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_encoder_fallback)
add_test(test_encoder_fallback ${CMAKE_CURRENT_BINARY_DIR}/test_encoder_fallback)
