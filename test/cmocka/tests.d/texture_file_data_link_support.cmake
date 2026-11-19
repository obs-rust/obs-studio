# Real image conversion and session glue for every standalone Rust consumer.
# Keep the unused private graphics callback in its own lazy archive member.
function(_texture_file_data_link_support)
  add_library(
    texture_file_data_link_support
    STATIC
    "${CMAKE_SOURCE_DIR}/libobs/build/libobs.media-texture-file-data-adapter+.h.c"
    "${CMAKE_SOURCE_DIR}/test/cmocka/texture-file-data-max-size-link-stub.c"
  )
  target_include_directories(texture_file_data_link_support PRIVATE "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs")
  target_link_libraries(
    texture_file_data_link_support
    PRIVATE
      OBS::libobs
      FFmpeg::avcodec
      FFmpeg::avformat
      FFmpeg::avutil
      FFmpeg::swscale
      "$<$<PLATFORM_ID:Windows>:rpcrt4>"
      "$<$<PLATFORM_ID:Windows>:winmm>"
  )
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if("libobs_rust-static" IN_LIST _links)
      target_link_libraries(${_target} PRIVATE texture_file_data_link_support)
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _texture_file_data_link_support)
