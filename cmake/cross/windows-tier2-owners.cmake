# Reuse each real plugin's canonical source/dependency/resource graph. The
# presentation hook omits native PDB/install copies unsupported by a GNU driver;
# resource copying and plugin build properties still come from the native helper.
list(PREPEND CMAKE_MODULE_PATH "${OBS_SOURCE_ROOT}/cmake/windows")
include("${OBS_SOURCE_ROOT}/cmake/windows/helpers.cmake")
function(set_target_properties_obs target)
  cmake_parse_arguments(PARSE_ARGV 1 _OWNER "" "" "PROPERTIES")
  set_property(TARGET ${target} PROPERTY PREFIX "")
  while(_OWNER_PROPERTIES)
    list(POP_FRONT _OWNER_PROPERTIES _key _value)
    set_property(TARGET ${target} PROPERTY ${_key} "${_value}")
  endwhile()
  # Native windows/compilerconfig.cmake supplies these directory-wide. The
  # cross-only owner hook preserves them without changing native options.
  target_compile_definitions(
    ${target}
    PRIVATE NOMINMAX UNICODE _UNICODE _CRT_SECURE_NO_WARNINGS _CRT_NONSTDC_NO_WARNINGS
  )
  target_install_resources(${target})
endfunction()
function(obs_windows_tier2_owners)
  set(CMAKE_SOURCE_DIR "${OBS_SOURCE_ROOT}")
  set(PROJECT_SOURCE_DIR "${OBS_SOURCE_ROOT}")
  set(OBS_SOURCE_DIR "${OBS_SOURCE_ROOT}")
  set(OBS_OUTPUT_DIR "${CMAKE_BINARY_DIR}/rundir")
  set(OBS_DATA_DESTINATION data)
  set(OBS_PLUGIN_DESTINATION core)
  set(CMAKE_VS_PLATFORM_NAME x64)
  # The producer builds this architecture on Linux. Native multiarchitecture
  # post-build commands must not start source-tree native Windows builds.
  set(OBS_PARENT_ARCHITECTURE cross-linux)
  set(CMAKE_FIND_PACKAGE_PREFER_CONFIG TRUE)
  set(ENABLE_HEVC ON)
  # Declare the real native libobs interface owners without rebuilding libobs.
  set(CMAKE_CURRENT_SOURCE_DIR "${OBS_SOURCE_ROOT}/libobs")
  include("${OBS_SOURCE_ROOT}/libobs/cmake/os-windows.cmake")
  # The native interface lists contain relative sources. Their owning CMake
  # directory here is cmake/cross; retain the original libobs source roots.
  foreach(_interface IN ITEMS obs-obfuscate obs-comutils obs-winhandle obs-threading-windows)
    get_target_property(_sources ${_interface} INTERFACE_SOURCES)
    string(REPLACE "${OBS_SOURCE_ROOT}/cmake/cross/" "${OBS_SOURCE_ROOT}/libobs/" _sources "${_sources}")
    set_property(TARGET ${_interface} PROPERTY INTERFACE_SOURCES "${_sources}")
  endforeach()
  foreach(_shared IN ITEMS obs-hook-config obs-inject-library ipc-util obs-d3d8-api)
    add_subdirectory("${OBS_SOURCE_ROOT}/shared/${_shared}" "${CMAKE_BINARY_DIR}/shared/${_shared}")
  endforeach()
  add_subdirectory("${OBS_SOURCE_ROOT}/libobs-winrt" "${CMAKE_BINARY_DIR}/libobs-winrt")
  # The MSVC SDK rejects Clang's experimental coroutine header. Use its
  # supported standard coroutine implementation for this cross-only module.
  set_target_properties(libobs-winrt PROPERTIES CXX_STANDARD 20 CXX_SCAN_FOR_MODULES OFF)
  # This unity group has no C++ modules. Match the native C++17 graph's lack
  # of module scanning after selecting the standard coroutine header.
  target_compile_options(libobs-winrt PRIVATE -fno-char8_t)
  foreach(
    _plugin
    IN
    ITEMS obs-outputs obs-filters image-source obs-transitions text-freetype2 win-capture
  )
    add_subdirectory("${OBS_SOURCE_ROOT}/plugins/${_plugin}" "${CMAKE_BINARY_DIR}/plugins/${_plugin}")
    if(TARGET ${_plugin})
      target_enable(${_plugin})
    endif()
  endforeach()
  # Match rnnoise.cmake's MSVC ABI typedef for GNU-style Clang as well.
  set_property(
    SOURCE "${OBS_SOURCE_ROOT}/plugins/obs-filters/build/obs-filters.noise-suppress+.h.c" TARGET_DIRECTORY obs-filters
    APPEND
    PROPERTY COMPILE_DEFINITIONS ssize_t=intptr_t
  )
  # Generate the real list for this enabled owner graph with the native template.
  get_property(_modules GLOBAL PROPERTY OBS_MODULES_ENABLED)
  list(LENGTH _modules OBS_CORE_MODULE_COUNT)
  string(REPLACE ";" "\",\n\t\"" _module_names "${_modules}")
  set(OBS_CORE_MODULE_LIST "\t\"${_module_names}\"")
  configure_file(
    "${OBS_SOURCE_ROOT}/libobs/obs-core-modules.c.in"
    "${CMAKE_BINARY_DIR}/config/obs-core-modules.c"
    @ONLY
  )
endfunction()
