# Rust-static step consumers can pull the decoder shim from a shared archive
# member. Compile the real, callback-based FFmpeg adapter; no decoder stubs.
function(_image_decode_link_support)
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if("libobs_rust-static" IN_LIST _links)
      get_target_property(_sources ${_target} SOURCES)
      set(_adapter "${CMAKE_SOURCE_DIR}/libobs/build/libobs.image-decode-adapter+.h.c")
      if(NOT _adapter IN_LIST _sources)
        target_sources(${_target} PRIVATE "${_adapter}")
        target_include_directories(${_target} PRIVATE "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs")
        target_link_libraries(${_target} PRIVATE FFmpeg::avcodec FFmpeg::avformat FFmpeg::avutil)
      endif()
      if(
        _target
          MATCHES
          "^test_(audio_monitor_delay|coreaudio_device_rules|coreaudio_pacing|monitor_speaker_layout|monitor_video_sync|monitor_wasapi|pulse_monitor)_steps_rust$"
        OR _target MATCHES "^test_(coreaudio_device_rules|monitor_speaker_layout|monitor_video_sync)_rust$"
      )
        target_sources(${_target} PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/source-getter-log-link-stub.c")
      endif()
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _image_decode_link_support)
