# Hidden-symbol C-source characterization with controlled dependency results.
# Extract the unchanged array/function; this does not exercise a Rust swap.
set(_source_init_context_path "${CMAKE_CURRENT_LIST_DIR}/../../../libobs/obs-source.c.h")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${_source_init_context_path}")
file(READ "${_source_init_context_path}" _source_init_context_text)
string(FIND "${_source_init_context_text}" "static const char *source_signals[]" _source_init_context_start)
string(FIND "${_source_init_context_text}" "\nconst char *obs_source_get_display_name(" _source_init_context_end)
if(_source_init_context_start LESS 0 OR _source_init_context_end LESS _source_init_context_start)
  message(FATAL_ERROR "Cannot locate source context characterization unit")
endif()
math(EXPR _source_init_context_length "${_source_init_context_end} - ${_source_init_context_start}")
string(
  SUBSTRING "${_source_init_context_text}"
  ${_source_init_context_start}
  ${_source_init_context_length}
  _source_init_context_body
)
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/source-init-context-characterization.inc" "${_source_init_context_body}")
include("${CMAKE_CURRENT_LIST_DIR}/../characterization-headers.cmake")
