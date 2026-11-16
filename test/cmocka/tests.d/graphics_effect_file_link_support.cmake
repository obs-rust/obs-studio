function(_graphics_effect_file_link_support)
  set(
    _graph_targets
    test_effect_param_types_steps_rust
    test_effect_parser_blocks_rust
    test_effect_parser_compile_rust
    test_effect_parser_lifecycle_rust
    test_effect_parser_params_rust
    test_effect_parser_passes_rust
    test_effect_parser_shader_call_rust
    test_graphics_ffmpeg_pq_steps_rust
    test_graphics_ffmpeg_route_steps_rust
    test_graphics_ffmpeg_wic_checks_rust
    test_graphics_immediate_steps_rust
    test_graphics_state_stacks_rust
    test_nsgif_decode_rust
    test_sprite_geometry_steps_rust
  )
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if("libobs_rust-static" IN_LIST _links AND _target IN_LIST _graph_targets)
      get_target_property(_sources ${_target} SOURCES)
      if(NOT "${_sources}" MATCHES "obs-graphics-effect-file-fields")
        target_sources(${_target} PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/graphics-effect-file-link-support.c")
        target_include_directories(${_target} PRIVATE "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs")
      endif()
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _graphics_effect_file_link_support)
