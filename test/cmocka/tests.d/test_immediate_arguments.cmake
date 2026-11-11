# GPU-free characterization of the real immediate argument adapter exports.
add_executable(test_immediate_arguments test_immediate_arguments.c)
target_include_directories(test_immediate_arguments PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_immediate_arguments PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES})
if(OS_WINDOWS)
  target_link_libraries(test_immediate_arguments PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_immediate_arguments)
add_test(NAME test_immediate_arguments COMMAND test_immediate_arguments)

# Record the adapters' exact downstream calls, including units outside the live
# array domain. Read the real bodies from their owning member before or after
# extraction; never keep a second hand-maintained copy of these functions.
set(_immediate_argument_source "")
foreach(_member graphics.c.h graphics-immediate-arguments.c.h)
  set(_path "${CMAKE_SOURCE_DIR}/libobs/graphics/${_member}")
  if(EXISTS "${_path}")
    set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${_path}")
    file(READ "${_path}" _text)
    string(APPEND _immediate_argument_source "\n${_text}")
  endif()
endforeach()
set(_immediate_argument_bodies "")
foreach(_function gs_vertex3f gs_texcoord gs_vertex2v)
  string(REGEX MATCH "void ${_function}\\([^}]+\n}" _body "${_immediate_argument_source}")
  if(NOT _body)
    message(FATAL_ERROR "Missing immediate adapter body: ${_function}")
  endif()
  string(APPEND _immediate_argument_bodies "${_body}\n\n")
endforeach()
configure_file(
  "${CMAKE_CURRENT_SOURCE_DIR}/immediate-arguments-probe.c.in"
  "${CMAKE_CURRENT_BINARY_DIR}/immediate-arguments-probe.c"
  @ONLY
)
target_sources(test_immediate_arguments PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/immediate-arguments-probe.c")
