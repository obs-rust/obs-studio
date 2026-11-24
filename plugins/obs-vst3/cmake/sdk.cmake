# Build the subset used by the OBS host and scanner. A local SDK can be
# supplied for offline builds. Packaging uses the pinned upstream revision.
set(STEINBERG_VST3_PATH "" CACHE PATH "Root of a Steinberg VST 3 SDK 3.8.0 checkout")
if(STEINBERG_VST3_PATH)
  set(_vst3_sdk "${STEINBERG_VST3_PATH}")
elseif(DEFINED ENV{STEINBERG_VST3_PATH} AND NOT "$ENV{STEINBERG_VST3_PATH}" STREQUAL "")
  set(_vst3_sdk "$ENV{STEINBERG_VST3_PATH}")
elseif(IS_DIRECTORY "${CMAKE_SOURCE_DIR}/.deps/vst3sdk/pluginterfaces")
  set(_vst3_sdk "${CMAKE_SOURCE_DIR}/.deps/vst3sdk")
else()
  include(FetchContent)
  FetchContent_Declare(
    obs_vst3_sdk
    GIT_REPOSITORY https://github.com/steinbergmedia/vst3sdk.git
    GIT_TAG 9fad9770f2ae8542ab1a548a68c1ad1ac690abe0
    GIT_SUBMODULES base pluginterfaces public.sdk SOURCE_SUBDIR obs-sdk-source-only
  )
  FetchContent_MakeAvailable(obs_vst3_sdk)
  set(_vst3_sdk "${obs_vst3_sdk_SOURCE_DIR}")
endif()

if(OS_MACOS)
  set(_vst3_host macos)
elseif(OS_WINDOWS)
  set(_vst3_host windows)
else()
  set(_vst3_host linux)
endif()

# The SDK is shared with the Cargo native build. Keep one source inventory.
set(_vst3_manifest_path "${CMAKE_CURRENT_SOURCE_DIR}/build/sdk-source-sets.json")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${_vst3_manifest_path}")
file(READ "${_vst3_manifest_path}" _vst3_manifest)
set(_vst3_sources)
foreach(_group IN ITEMS common "${_vst3_host}")
  string(JSON _count LENGTH "${_vst3_manifest}" "${_group}")
  math(EXPR _last "${_count} - 1")
  foreach(_index RANGE "${_last}")
    string(JSON _path GET "${_vst3_manifest}" "${_group}" "${_index}")
    if(NOT EXISTS "${_vst3_sdk}/${_path}")
      message(FATAL_ERROR "VST3 SDK source missing: ${_vst3_sdk}/${_path}. Check SDK submodules.")
    endif()
    list(APPEND _vst3_sources "${_vst3_sdk}/${_path}")
  endforeach()
endforeach()

add_library(obs-vst3-sdk STATIC ${_vst3_sources})
add_library(OBS::vst3-sdk ALIAS obs-vst3-sdk)
target_include_directories(obs-vst3-sdk SYSTEM PUBLIC "${_vst3_sdk}")
target_link_libraries(
  obs-vst3-sdk
  PUBLIC
    ${CMAKE_DL_LIBS}
    "$<$<PLATFORM_ID:Darwin>:$<LINK_LIBRARY:FRAMEWORK,Cocoa.framework>>"
    "$<$<PLATFORM_ID:Darwin>:$<LINK_LIBRARY:FRAMEWORK,Foundation.framework>>"
)
set_target_properties(
  obs-vst3-sdk
  PROPERTIES FOLDER plugins/obs-vst3 POSITION_INDEPENDENT_CODE ON COMPILE_WARNING_AS_ERROR OFF
)
target_compile_options(obs-vst3-sdk PRIVATE "$<$<COMPILE_LANG_AND_ID:OBJCXX,AppleClang>:-fobjc-arc>")
if(OS_MACOS)
  set_target_properties(
    obs-vst3-sdk
    PROPERTIES XCODE_ATTRIBUTE_CLANG_ENABLE_OBJC_ARC YES XCODE_ATTRIBUTE_GCC_TREAT_WARNINGS_AS_ERRORS NO
  )
  set_source_files_properties(
    "${_vst3_sdk}/public.sdk/source/vst/hosting/module_mac.mm"
    PROPERTIES COMPILE_OPTIONS "-Wno-unused-parameter"
  )
endif()
