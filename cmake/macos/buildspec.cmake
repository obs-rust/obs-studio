# OBS CMake macOS build dependencies module

include_guard(GLOBAL)

include(buildspec_common)

# _obs_dependency_platform_hook: CEF ships per-arch macOS slices, not a
# universal one (#2994, #3016). Called by _check_dependencies; sets arch and
# platform (and skip_dependency) in its scope.
macro(_obs_dependency_platform_hook dependency)
  if("${dependency}" STREQUAL "cef" AND arch STREQUAL universal)
    if(CMAKE_OSX_ARCHITECTURES MATCHES ".+;.+")
      set(skip_dependency TRUE)
    else()
      if(CMAKE_OSX_ARCHITECTURES MATCHES "(arm64|x86_64)")
        set(arch ${CMAKE_OSX_ARCHITECTURES})
      else()
        set(arch ${CMAKE_HOST_SYSTEM_PROCESSOR})
      endif()
      set(platform macos-${arch})
    endif()
  endif()
endmacro()

# _check_dependencies_macos: Set up macOS slice for _check_dependencies
function(_check_dependencies_macos)
  set(arch universal)
  set(platform macos-${arch})

  set(dependencies_dir "${CMAKE_CURRENT_SOURCE_DIR}/.deps")
  set(prebuilt_filename "macos-deps-VERSION-ARCH-REVISION.tar.xz")
  set(prebuilt_destination "obs-deps-VERSION-ARCH")
  set(qt6_filename "macos-deps-qt6-VERSION-ARCH-REVISION.tar.xz")
  set(qt6_destination "obs-deps-qt6-VERSION-ARCH")
  set(cef_filename "cef_binary_VERSION_macos_ARCH_REVISION.tar.xz")
  set(cef_destination "cef_binary_VERSION_macos_ARCH")
  set(dependencies_list prebuilt qt6 cef)

  _check_dependencies(${dependencies_list})

  execute_process(
    COMMAND "xattr" -r -d com.apple.quarantine "${dependencies_dir}/${destination}"
    RESULT_VARIABLE result
    COMMAND_ERROR_IS_FATAL ANY
  )
endfunction()

_check_dependencies_macos()
