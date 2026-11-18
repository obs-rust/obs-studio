# Real image conversion and session glue for every standalone Rust consumer.
function(_texture_file_data_link_support)
  add_library(
    texture_file_data_link_support
    STATIC
    "${CMAKE_SOURCE_DIR}/libobs/build/libobs.media-texture-file-data-adapter+.h.c"
  )
  target_include_directories(texture_file_data_link_support PRIVATE "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs")
  target_link_libraries(
    texture_file_data_link_support
    PRIVATE OBS::libobs FFmpeg::avcodec FFmpeg::avformat FFmpeg::avutil FFmpeg::swscale
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
