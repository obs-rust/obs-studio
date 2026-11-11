# Consume only this owner's canonical entry here: the executable predates
# ENABLE_UNIT_TESTS and must exist before the parent's compile-options calls.
set(HAVE_X264_TEST_OWNER TRUE)
set(C_COMPILER_CLANG FALSE)
if(CMAKE_C_COMPILER_ID MATCHES "Clang")
  set(C_COMPILER_CLANG TRUE)
endif()
include("${CMAKE_SOURCE_DIR}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${CMAKE_SOURCE_DIR}/test/cmocka/build/test-graph.d/obs-x264-test.json" "${CMAKE_SOURCE_DIR}")
set_target_properties(obs-x264-test PROPERTIES FOLDER plugins/obs-x264)
