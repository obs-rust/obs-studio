# Standalone Rust-static consumers need the real private FFmpeg adapter too.
# Extract only its PCH include, so these tests do not inherit all media-unity
# dependencies. Every operation below remains the production implementation.
function(_ffmpeg_image_init_link_support)
  set(_source "${CMAKE_SOURCE_DIR}/libobs/graphics/graphics-ffmpeg-init-adapter.c.h")
  set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${_source}")
  file(READ "${_source}" _body)
  string(REPLACE "#include \"build/libobs-media.pch.h\"" "" _body "${_body}")
  set(_adapter "${CMAKE_CURRENT_BINARY_DIR}/ffmpeg-image-init-link-support.c")
  file(
    WRITE "${_adapter}"
    "#include <errno.h>\n#include <inttypes.h>\n#include <string.h>\n#include \"util/base.h\"\n#include <libavcodec/avcodec.h>\n#include <libavformat/avformat.h>\n#include \"graphics/graphics.h\"\n#include \"graphics/graphics-ffmpeg-orient.h\"\n#include \"graphics/graphics-ffmpeg-init-private.h\"\n${_body}"
  )
  # Keep this as a lazy archive member. Adding the object directly to every
  # executable makes its image-size call pull unrelated Rust graphics members
  # into pure source/scene/monitor fixtures, even when they never decode images.
  add_library(ffmpeg_image_init_link_support STATIC "${_adapter}")
  target_include_directories(ffmpeg_image_init_link_support PRIVATE "${CMAKE_SOURCE_DIR}/libobs")
  target_link_libraries(ffmpeg_image_init_link_support PRIVATE FFmpeg::avcodec FFmpeg::avformat FFmpeg::avutil)
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if("libobs_rust-static" IN_LIST _links)
      target_link_libraries(${_target} PRIVATE ffmpeg_image_init_link_support)
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _ffmpeg_image_init_link_support)
