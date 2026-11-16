# C-source characterization only: compile the real bodies with dependency seams.
set(_encoder_group_source "${CMAKE_SOURCE_DIR}/libobs/obs-encoder-group-lifecycle.c.h")
if(NOT EXISTS "${_encoder_group_source}")
  set(_encoder_group_source "${CMAKE_SOURCE_DIR}/libobs/obs-encoder.c.h")
endif()
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${_encoder_group_source}")
file(READ "${_encoder_group_source}" _encoder_group_text)
set(_encoder_group_header "")
foreach(
  _encoder_group_name
  IN
  ITEMS obs_encoder_group_create obs_encoder_group_actually_destroy obs_encoder_group_destroy
)
  # Match definitions, not the forward declaration or earlier calls in obs-encoder.c.h.
  if(_encoder_group_name STREQUAL "obs_encoder_group_create")
    set(_encoder_group_signature "obs_encoder_group_t *${_encoder_group_name}(void)\n{")
  else()
    set(_encoder_group_signature "void ${_encoder_group_name}(obs_encoder_group_t *group)\n{")
  endif()
  string(FIND "${_encoder_group_text}" "${_encoder_group_signature}" _encoder_group_start)
  if(_encoder_group_start LESS 0)
    message(FATAL_ERROR "Cannot locate definition of ${_encoder_group_name}")
  endif()
  string(SUBSTRING "${_encoder_group_text}" ${_encoder_group_start} -1 _encoder_group_tail)
  string(FIND "${_encoder_group_tail}" "\n}\n" _encoder_group_end)
  if(_encoder_group_end LESS 0)
    message(FATAL_ERROR "Cannot locate end of ${_encoder_group_name}")
  endif()
  math(EXPR _encoder_group_length "${_encoder_group_end} + 3")
  string(SUBSTRING "${_encoder_group_tail}" 0 ${_encoder_group_length} _encoder_group_body)
  string(APPEND _encoder_group_header "static ${_encoder_group_body}\n")
endforeach()
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/encoder-group-lifecycle-under-test.h" "${_encoder_group_header}")
add_executable(test_encoder_group_lifecycle test_encoder_group_lifecycle.c)
target_include_directories(test_encoder_group_lifecycle PRIVATE "${CMAKE_CURRENT_BINARY_DIR}")
target_link_libraries(
  test_encoder_group_lifecycle
  PRIVATE OBS::libobs OBS::caption Uthash::Uthash OBS::native-test-cmocka
)
if(OS_WINDOWS)
  target_link_libraries(test_encoder_group_lifecycle PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_encoder_group_lifecycle)
add_test(NAME test_encoder_group_lifecycle COMMAND test_encoder_group_lifecycle)
