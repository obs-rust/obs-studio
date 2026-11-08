# Preserve the original C characterization unchanged after the intentional
# #1275 behavior correction. This target now exercises the frozen baseline;
# test_lua_graphics_lock_bounded compiles and tests the production helper.
configure_file(
  "${CMAKE_CURRENT_LIST_DIR}/../../../test/cmocka/fixtures/lua-graphics-lock-original.h"
  "${CMAKE_CURRENT_BINARY_DIR}/lua-graphics-lock-under-test.h"
  COPYONLY
)

# Generated include interface used by ../build/test-graph.json.
include("${CMAKE_CURRENT_LIST_DIR}/../characterization-headers.cmake")
