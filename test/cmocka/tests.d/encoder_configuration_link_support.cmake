# A Rust archive member can pull these setters into unrelated source-step tests.
# Keep the real adapter in an archive: adding its object to every executable
# eagerly pulls video-output and its host dependencies into unrelated tests.
function(_encoder_configuration_link_support)
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if("libobs_rust-static" IN_LIST _links)
      if(NOT TARGET encoder_configuration_link_support)
        add_library(
          encoder_configuration_link_support
          STATIC
          "${CMAKE_CURRENT_SOURCE_DIR}/encoder-configuration-link-support.c"
        )
        target_include_directories(
          encoder_configuration_link_support
          PRIVATE "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_BINARY_DIR}/config"
        )
        target_link_libraries(
          encoder_configuration_link_support
          PRIVATE OBS::caption Uthash::Uthash $<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>
        )
      endif()
      target_link_libraries(${_target} PRIVATE encoder_configuration_link_support)
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _encoder_configuration_link_support)
