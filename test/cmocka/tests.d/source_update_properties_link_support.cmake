# Archive real adapters so fixture-owned definitions take precedence. Scan all
# Rust-static consumers, including unrelated tests and consumers of OBS::libobs.
function(_source_update_properties_link_support)
  add_library(
    source_update_properties_link_support
    STATIC
    "${CMAKE_CURRENT_SOURCE_DIR}/source-update-properties-link-support.c"
  )
  target_include_directories(
    source_update_properties_link_support
    PRIVATE "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_BINARY_DIR}/config"
  )
  target_link_libraries(
    source_update_properties_link_support
    PRIVATE OBS::caption Uthash::Uthash "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
  )
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if("libobs_rust-static" IN_LIST _links)
      target_link_libraries(${_target} PRIVATE source_update_properties_link_support)
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _source_update_properties_link_support)
