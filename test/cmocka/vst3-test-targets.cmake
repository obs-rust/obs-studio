add_library(vst3-test-gain MODULE EXCLUDE_FROM_ALL "${CMAKE_CURRENT_LIST_DIR}/vst3-test-gain.cpp")
target_link_libraries(vst3-test-gain PRIVATE OBS::vst3-sdk)
set_target_properties(vst3-test-gain PROPERTIES PREFIX "" OUTPUT_NAME OBSFixture)

add_executable(
  test_vst3_host
  EXCLUDE_FROM_ALL
  "${CMAKE_CURRENT_LIST_DIR}/test_vst3_host.c"
  "${CMAKE_CURRENT_LIST_DIR}/vst3-test-qt.cpp"
)
target_compile_definitions(test_vst3_host PRIVATE "OBS_VST3_TEST_X11=$<BOOL:${OS_LINUX}>")
target_link_libraries(test_vst3_host PRIVATE OBS::libobs Qt::Widgets)
