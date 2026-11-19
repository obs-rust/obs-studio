include("${CMAKE_CURRENT_LIST_DIR}/../write-if-different.cmake")
# Link the real private adapter only when a Rust-static consumer needs it.
file(READ "${CMAKE_SOURCE_DIR}/libobs/obs-video-level-queries-fields.c.h" _video_level_queries_adapter)
string(
  REPLACE
  "#include \"libobs/build/libobs.pch.h\""
  ""
  _video_level_queries_adapter
  "${_video_level_queries_adapter}"
)
obs_test_write_file("${CMAKE_CURRENT_BINARY_DIR}/video-level-queries-fields.c" "#include \"obs-internal.h\"\n${_video_level_queries_adapter}"
)
function(_video_level_queries_link_support)
  add_library(video_level_queries_link_support STATIC "${CMAKE_CURRENT_BINARY_DIR}/video-level-queries-fields.c")
  target_include_directories(
    video_level_queries_link_support
    PRIVATE "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_BINARY_DIR}/config"
  )
  target_link_libraries(
    video_level_queries_link_support
    PRIVATE OBS::caption Uthash::Uthash "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
  )
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if("libobs_rust-static" IN_LIST _links)
      target_link_libraries(${_target} PRIVATE video_level_queries_link_support)
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _video_level_queries_link_support)
