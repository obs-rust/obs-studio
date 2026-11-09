# Keep the real, global-dependent adapter in its own archive member: unrelated
# Rust-static step fixtures do not own a live OBS core and never call the setter.
function(_source_audio_active_setter_link_support)
  add_library(
    source-audio-active-setter-link-support
    STATIC
    "${CMAKE_CURRENT_SOURCE_DIR}/source-audio-active-setter-link-support.c"
  )
  target_include_directories(source-audio-active-setter-link-support PRIVATE "${CMAKE_SOURCE_DIR}/libobs")
  target_link_libraries(
    source-audio-active-setter-link-support
    PRIVATE OBS::caption Uthash::Uthash "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
  )
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if(_target MATCHES "^test_(async_|deinterlace_|source_|transition_)" AND "libobs_rust-static" IN_LIST _links)
      target_link_libraries(${_target} PRIVATE source-audio-active-setter-link-support)
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _source_audio_active_setter_link_support)
