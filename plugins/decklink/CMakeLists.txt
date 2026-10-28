cmake_minimum_required(VERSION 3.28...3.30)

option(ENABLE_DECKLINK "Build OBS with Decklink support" ON)
if(NOT ENABLE_DECKLINK)
  return()
endif()

add_library(decklink MODULE)
add_library(OBS::decklink ALIAS decklink)
obs_msvc_exempt_c4201(decklink)
# MSVC /W4 (#2626): FrameQueue pads its nodes to a cache line so producers and
# consumers do not share one (decklink-device-instance.hpp), so the padding
# C4324 reports there is the intent, not slack in a struct layout.
target_compile_options(decklink PRIVATE $<$<COMPILE_LANG_AND_ID:CXX,MSVC>:/wd4324>)

add_library(decklink-sdk INTERFACE)
add_library(Decklink::SDK ALIAS decklink-sdk)

# platform.hpp is a shared shim; SDK types/includes come from one host file.
target_include_directories(
  decklink-sdk
  INTERFACE
    "$<$<PLATFORM_ID:Windows>:${CMAKE_CURRENT_SOURCE_DIR}/hosts/windows>"
    "$<$<PLATFORM_ID:Darwin>:${CMAKE_CURRENT_SOURCE_DIR}/hosts/macos>"
    "$<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:${CMAKE_CURRENT_SOURCE_DIR}/hosts/linux>"
)

target_sources(
  decklink-sdk
  INTERFACE
    "$<$<PLATFORM_ID:Darwin>:${CMAKE_CURRENT_SOURCE_DIR}/mac/decklink-sdk/DeckLinkAPI.h>"
    "$<$<PLATFORM_ID:Darwin>:${CMAKE_CURRENT_SOURCE_DIR}/mac/decklink-sdk/DeckLinkAPIConfiguration.h>"
    "$<$<PLATFORM_ID:Darwin>:${CMAKE_CURRENT_SOURCE_DIR}/mac/decklink-sdk/DeckLinkAPIDeckControl.h>"
    "$<$<PLATFORM_ID:Darwin>:${CMAKE_CURRENT_SOURCE_DIR}/mac/decklink-sdk/DeckLinkAPIDiscovery.h>"
    "$<$<PLATFORM_ID:Darwin>:${CMAKE_CURRENT_SOURCE_DIR}/mac/decklink-sdk/DeckLinkAPIDispatch.cpp>"
    "$<$<PLATFORM_ID:Darwin>:${CMAKE_CURRENT_SOURCE_DIR}/mac/decklink-sdk/DeckLinkAPIModes.h>"
    "$<$<PLATFORM_ID:Darwin>:${CMAKE_CURRENT_SOURCE_DIR}/mac/decklink-sdk/DeckLinkAPIStreaming.h>"
    "$<$<PLATFORM_ID:Darwin>:${CMAKE_CURRENT_SOURCE_DIR}/mac/decklink-sdk/DeckLinkAPITypes.h>"
    "$<$<PLATFORM_ID:Darwin>:${CMAKE_CURRENT_SOURCE_DIR}/mac/decklink-sdk/DeckLinkAPIVersion.h>"
    "$<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:${CMAKE_CURRENT_SOURCE_DIR}/linux/decklink-sdk/DeckLinkAPI.h>"
    "$<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:${CMAKE_CURRENT_SOURCE_DIR}/linux/decklink-sdk/DeckLinkAPIConfiguration.h>"
    "$<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:${CMAKE_CURRENT_SOURCE_DIR}/linux/decklink-sdk/DeckLinkAPIDeckControl.h>"
    "$<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:${CMAKE_CURRENT_SOURCE_DIR}/linux/decklink-sdk/DeckLinkAPIDiscovery.h>"
    "$<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:${CMAKE_CURRENT_SOURCE_DIR}/linux/decklink-sdk/DeckLinkAPIDispatch.cpp>"
    "$<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:${CMAKE_CURRENT_SOURCE_DIR}/linux/decklink-sdk/DeckLinkAPIModes.h>"
    "$<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:${CMAKE_CURRENT_SOURCE_DIR}/linux/decklink-sdk/DeckLinkAPITypes.h>"
    "$<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:${CMAKE_CURRENT_SOURCE_DIR}/linux/decklink-sdk/DeckLinkAPIVersion.h>"
    "$<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:${CMAKE_CURRENT_SOURCE_DIR}/linux/decklink-sdk/LinuxCOM.h>"
    "$<$<PLATFORM_ID:Windows>:${CMAKE_CURRENT_SOURCE_DIR}/win/decklink-sdk/DeckLinkAPIVersion.h>"
)

target_sources(
  decklink
  PRIVATE
    $<$<PLATFORM_ID:Darwin>:build/decklink.macos+.hpp.cpp>
    $<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:build/decklink.linux+.hpp.cpp>
    $<$<PLATFORM_ID:Windows>:build/decklink.windows+.hpp.cpp>
    build/decklink.core+.hpp.cpp
    build/decklink.audio-repack+.h.c
    audio-repack.h
    audio-repack.hpp
    const.h
    decklink-device-discovery.hpp
    decklink-device-instance.hpp
    decklink-device-mode.hpp
    decklink-device.hpp
    decklink-devices.hpp
    DecklinkBase.h
    DecklinkInput.hpp
    DecklinkOutput.hpp
    OBSVideoFrame.h
    platform.hpp
    util.hpp
)

target_link_libraries(
  decklink
  PRIVATE
    OBS::libobs
    OBS::caption
    Decklink::SDK
    # it dlopen()s the DeckLink driver: libdl before glibc 2.34
    ${CMAKE_DL_LIBS}
    "$<$<PLATFORM_ID:Darwin>:$<LINK_LIBRARY:FRAMEWORK,CoreFoundation.framework>>"
)

# #2625: this target's conversions are fixed at the call sites, so the pins that
# hide them are not needed here. The global -Wno-conversion / -Wno-sign-conversion /
# -Wno-float-conversion only leave the flag lists once every target is ratcheted.
target_compile_options(
  decklink
  PRIVATE "$<$<COMPILE_LANG_AND_ID:C,GNU,Clang,AppleClang>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)

if(OS_WINDOWS)
  configure_file(cmake/windows/obs-module.rc.in win-decklink.rc)
  target_sources(decklink PRIVATE win-decklink.rc)

  include(idlfilehelper)
  target_add_idl_files(decklink win/decklink-sdk/DeckLinkAPI.idl)
  set_source_files_properties(win/decklink-sdk/DeckLinkAPIVersion.h PROPERTIES GENERATED TRUE)
  target_sources(decklink-sdk INTERFACE win/decklink-sdk/DeckLinkAPIVersion.h)
endif()

set_target_properties_obs(decklink PROPERTIES FOLDER plugins/decklink PREFIX "")
