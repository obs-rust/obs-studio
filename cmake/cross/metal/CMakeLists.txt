# A Swift-only Linux producer project. Enabling Swift after C/ObjC in CMake
# requires install_name_tool even when no install/rewrite is requested. This
# project links the final install name and signature in one Linux link instead.
cmake_minimum_required(VERSION 3.28...3.30)
if(NOT CMAKE_HOST_SYSTEM_NAME STREQUAL "Linux")
  message(FATAL_ERROR "The Metal Tier 2 producer compiles only on Linux")
endif()
# Swift's default static probe invokes Apple's libtool. Archive its real Darwin
# objects with the managed LLVM archiver instead; the actual Metal target still
# exercises swiftc's shared-library link. Nothing executes on the producer.
set(_metal_toolchain "${CMAKE_BINARY_DIR}/metal-toolchain.cmake")
if(NOT CMAKE_TOOLCHAIN_FILE STREQUAL _metal_toolchain)
  set(OBS_TIER2_PARENT_TOOLCHAIN "${CMAKE_TOOLCHAIN_FILE}" CACHE FILEPATH "Managed parent Darwin toolchain" FORCE)
endif()
if(NOT OBS_TIER2_PARENT_TOOLCHAIN)
  message(FATAL_ERROR "Metal requires the managed parent Darwin toolchain")
endif()
file(
  WRITE "${_metal_toolchain}"
  "include(\"${OBS_TIER2_PARENT_TOOLCHAIN}\")\nset(CMAKE_Swift_CREATE_STATIC_LIBRARY \"<CMAKE_AR> crs <TARGET> <OBJECTS>\")\n"
)
set(CMAKE_TOOLCHAIN_FILE "${_metal_toolchain}" CACHE FILEPATH "Swift link-probe toolchain" FORCE)
project(obs-tier2-metal LANGUAGES Swift)
set(CMAKE_BUILD_WITH_INSTALL_RPATH TRUE)
set(CMAKE_INSTALL_RPATH "@loader_path")
set(CMAKE_LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}")
add_library(libobs SHARED IMPORTED GLOBAL)
add_library(OBS::libobs ALIAS libobs)
set_target_properties(
  libobs
  PROPERTIES
    IMPORTED_LOCATION "${OBS_TIER2_CORE}"
    INTERFACE_INCLUDE_DIRECTORIES
      "${OBS_SOURCE_ROOT};${OBS_SOURCE_ROOT}/libobs;${OBS_TIER2_CONFIG};${OBS_TIER2_DEPS}/include"
)
function(target_enable_feature)
endfunction()
function(set_target_properties_obs target)
  cmake_parse_arguments(PARSE_ARGV 1 _props "" "" "PROPERTIES")
  while(_props_PROPERTIES)
    list(POP_FRONT _props_PROPERTIES _key _value)
    set_property(TARGET ${target} PROPERTY ${_key} "${_value}")
  endwhile()
  target_link_options(${target} PRIVATE "LINKER:-adhoc_codesign")
endfunction()
function(set_target_xcode_properties target)
  cmake_parse_arguments(PARSE_ARGV 1 _swift "" "" "PROPERTIES")
  while(_swift_PROPERTIES)
    list(POP_FRONT _swift_PROPERTIES _key _value)
    if(_key STREQUAL "SWIFT_OBJC_BRIDGING_HEADER")
      target_compile_options(${target} PRIVATE -import-objc-header "${_value}")
    elseif(_key STREQUAL "SWIFT_VERSION")
      set_property(TARGET ${target} PROPERTY Swift_LANGUAGE_VERSION 6)
    endif()
  endwhile()
  target_compile_options(${target} PRIVATE "SHELL:-Xcc -fobjc-arc" "SHELL:-Xcc -fmodules" "SHELL:-Xcc -std=gnu17")
endfunction()
add_subdirectory("${OBS_SOURCE_ROOT}/libobs-metal" "${CMAKE_BINARY_DIR}/recipe")
set_target_properties(libobs-metal PROPERTIES Swift_MODULE_NAME libobs_metal)
