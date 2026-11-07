# #1275 production regression, separate from the preserved characterization.
set(_lua_bounded_source "${CMAKE_CURRENT_LIST_DIR}/../../../shared/obs-scripting/obs-scripting-lua-source.c.h")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${_lua_bounded_source}")
file(READ "${_lua_bounded_source}" _lua_bounded_text)
string(FIND "${_lua_bounded_text}" "static bool lock_in_graphics(pthread_mutex_t *mutex)" _lua_bounded_start)
string(FIND "${_lua_bounded_text}" "#define lock_script_in_graphics(skip)" _lua_bounded_end)
if(_lua_bounded_start LESS 0 OR _lua_bounded_end LESS_EQUAL _lua_bounded_start)
  message(FATAL_ERROR "Cannot locate production lock_in_graphics for regression")
endif()
math(EXPR _lua_bounded_length "${_lua_bounded_end} - ${_lua_bounded_start}")
string(SUBSTRING "${_lua_bounded_text}" ${_lua_bounded_start} ${_lua_bounded_length} _lua_bounded_function)
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/lua-graphics-lock-bounded-under-test.h" "${_lua_bounded_function}")

# Generated include interface used by ../build/test-graph.json.
include("${CMAKE_CURRENT_LIST_DIR}/../characterization-headers.cmake")
