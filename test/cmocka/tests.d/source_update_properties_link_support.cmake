# Archive real adapters so fixture-owned definitions take precedence. Scan all
# Rust-static consumers, including unrelated tests and consumers of OBS::libobs.
function(_source_update_properties_link_support)
  # Calldata allocation can pull obs-util host members into unrelated consumers.
  if(TARGET gio::gio)
    find_package(PkgConfig REQUIRED)
    pkg_check_modules(_source_update_properties_gio REQUIRED IMPORTED_TARGET gio-2.0 glib-2.0 gobject-2.0)
  endif()
  add_library(
    source_update_properties_link_support
    STATIC
    "${CMAKE_CURRENT_SOURCE_DIR}/source-update-properties-link-support.c"
    "${CMAKE_CURRENT_SOURCE_DIR}/source-update-properties-base-log-link-support.c"
    "${CMAKE_CURRENT_SOURCE_DIR}/source-update-properties-base-crash-link-support.c"
  )
  # Each real service must remain independently demand-linked.
  set_target_properties(source_update_properties_link_support PROPERTIES UNITY_BUILD OFF)
  target_include_directories(
    source_update_properties_link_support
    PRIVATE "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_BINARY_DIR}/config"
  )
  target_link_libraries(
    source_update_properties_link_support
    PRIVATE OBS::caption Uthash::Uthash SIMDe::SIMDe "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
  )
  foreach(_host_library IN ITEMS LibUUID::LibUUID PkgConfig::_source_update_properties_gio)
    if(TARGET ${_host_library})
      target_link_libraries(source_update_properties_link_support PRIVATE ${_host_library})
    endif()
  endforeach()
  target_link_libraries(source_update_properties_link_support PRIVATE "$<$<PLATFORM_ID:Windows>:Rpcrt4;winmm>")
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if("libobs_rust-static" IN_LIST _links)
      target_link_libraries(${_target} PRIVATE source_update_properties_link_support)
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _source_update_properties_link_support)
