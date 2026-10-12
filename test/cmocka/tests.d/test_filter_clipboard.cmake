# Paste Filters pastes into one of the selected sources only
# (obsproject/obs-studio#4290). Builds the plain C helper the frontend calls,
# so the test does not link the Qt frontend. It needs no video; obs_startup()
# needs a display on Linux, so there the cases skip without one.
add_executable(test_filter_clipboard test_filter_clipboard.c "${CMAKE_SOURCE_DIR}/frontend/utility/filter-clipboard.c")
target_include_directories(test_filter_clipboard PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility")
target_link_libraries(test_filter_clipboard PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_filter_clipboard)
add_test(test_filter_clipboard ${CMAKE_CURRENT_BINARY_DIR}/test_filter_clipboard)
