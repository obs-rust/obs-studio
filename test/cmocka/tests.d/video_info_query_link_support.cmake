# Keep global-dependent adapters in an archive: fixture-owned cores take precedence.
# Shared libobs hides these private adapters; every Rust-static consumer needs
# the support archive, including consumers that also link OBS::libobs.
function(_video_info_query_link_support)
  add_library(
    video_info_query_link_support
    STATIC
    "${CMAKE_CURRENT_SOURCE_DIR}/video-info-query-link-support.c"
    "${CMAKE_CURRENT_SOURCE_DIR}/core-state-link-support.c"
    "${CMAKE_CURRENT_SOURCE_DIR}/encoder-video-tex-active-fields-link-support.c"
    "${CMAKE_CURRENT_SOURCE_DIR}/encoder-video-tex-active-mix-link-support.c"
    "${CMAKE_CURRENT_SOURCE_DIR}/sceneitem-defer-update-link-support.c"
  )
  target_include_directories(
    video_info_query_link_support
    PRIVATE "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_BINARY_DIR}/config"
  )
  target_link_libraries(
    video_info_query_link_support
    PRIVATE OBS::caption Uthash::Uthash $<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>
  )
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if("libobs_rust-static" IN_LIST _links)
      target_link_libraries(${_target} PRIVATE video_info_query_link_support)
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _video_info_query_link_support)
