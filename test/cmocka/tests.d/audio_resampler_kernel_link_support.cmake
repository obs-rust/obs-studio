# Rust owns the wrapper, while this real C adapter retains FFmpeg's version
# boundary. Legacy Rust-static source-step consumers need that adapter too.
function(_audio_resampler_kernel_link_support)
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if("libobs_rust-static" IN_LIST _links)
      target_sources(${_target} PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/audio-resampler-kernel-link-support.c")
      target_include_directories(${_target} PRIVATE "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs")
      target_link_libraries(${_target} PRIVATE FFmpeg::swresample FFmpeg::avutil)
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _audio_resampler_kernel_link_support)
