# Profile and Scene Collection menu entries show the names as typed ('&' is a
# Qt mnemonic marker). Builds the plain C helper the frontend calls, so the
# test needs neither Qt nor a display.
add_executable(test_menu_text test_menu_text.c "${CMAKE_SOURCE_DIR}/frontend/utility/menu-text.c")
target_include_directories(test_menu_text PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility")
target_link_libraries(test_menu_text PRIVATE ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_menu_text)
add_test(test_menu_text ${CMAKE_CURRENT_BINARY_DIR}/test_menu_text)
