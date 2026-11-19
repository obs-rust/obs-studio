# The Rust obs_get_module_allow_disable() names the generated
# obs_in_core_module_list(). Standalone Rust-static consumers get the real
# generated matcher as an archive, so a fixture pulls it only when its archive
# member needs it. The archive compiles the generated source that
# OBS::libobs-core-modules owns (configured even with ENABLE_CORE_MODULES off;
# the cross build aliases its own generated target), rather than reusing that
# target's objects: Xcode does not build a library made only of other
# targets' objects.
function(_module_disable_query_link_support)
  if(NOT TARGET OBS::libobs-core-modules)
    return()
  endif()
  get_target_property(_core_modules OBS::libobs-core-modules ALIASED_TARGET)
  get_target_property(_core_sources ${_core_modules} SOURCES)
  list(FILTER _core_sources INCLUDE REGEX "obs-core-modules\\.c$")
  set_source_files_properties(${_core_sources} PROPERTIES GENERATED TRUE)
  get_directory_property(_targets BUILDSYSTEM_TARGETS)
  foreach(_target IN LISTS _targets)
    get_target_property(_links ${_target} LINK_LIBRARIES)
    if("libobs_rust-static" IN_LIST _links)
      if(NOT TARGET module_disable_query_link_support)
        add_library(module_disable_query_link_support STATIC ${_core_sources})
        target_include_directories(
          module_disable_query_link_support
          PRIVATE "${CMAKE_CURRENT_FUNCTION_LIST_DIR}/../../../libobs"
        )
        set_target_properties(module_disable_query_link_support PROPERTIES POSITION_INDEPENDENT_CODE TRUE)
      endif()
      target_link_libraries(${_target} PRIVATE module_disable_query_link_support)
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL _module_disable_query_link_support)
