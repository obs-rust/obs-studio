# Controlled dependencies exercise both the original C unit and the private Rust
# shim in separate executables. Both compile the unchanged characterization test.
set(_source_init_context_path "${CMAKE_CURRENT_LIST_DIR}/../../../libobs/obs-source-init-context.c.h")
cmake_path(NORMAL_PATH _source_init_context_path)
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${_source_init_context_path}")
file(READ "${_source_init_context_path}" _source_init_context_text)
string(FIND "${_source_init_context_text}" "static const char *source_signals[]" _source_init_context_start)
string(LENGTH "${_source_init_context_text}" _source_init_context_end)
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

file(MAKE_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/source-init-context-rust")
# Reuse the production adapter with the test's existing dependency macros.
set(_source_init_context_adapter "${CMAKE_CURRENT_LIST_DIR}/../../../libobs/obs-source-init-context-fields.c.h")
cmake_path(NORMAL_PATH _source_init_context_adapter)
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${_source_init_context_adapter}")
file(READ "${_source_init_context_adapter}" _source_init_context_adapter_text)
string(
  REPLACE
  "#include \"libobs/build/libobs.pch.h\""
  ""
  _source_init_context_adapter_text
  "${_source_init_context_adapter_text}"
)
file(
  WRITE "${CMAKE_CURRENT_BINARY_DIR}/source-init-context-rust/source-init-context-characterization.inc"
  "${_source_init_context_adapter_text}"
)
add_library(OBS::source-init-context-rust-headers INTERFACE IMPORTED)
set_target_properties(
  OBS::source-init-context-rust-headers
  PROPERTIES INTERFACE_INCLUDE_DIRECTORIES "${CMAKE_CURRENT_BINARY_DIR}/source-init-context-rust"
)
