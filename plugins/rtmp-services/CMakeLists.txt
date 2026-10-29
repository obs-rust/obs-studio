cmake_minimum_required(VERSION 3.28...3.30)

option(ENABLE_SERVICE_UPDATES "Checks for service updates" ON)

set(
  RTMP_SERVICES_URL
  "https://obsproject.com/obs2_update/rtmp-services"
  CACHE STRING
  "Default services package URL"
  FORCE
)
mark_as_advanced(RTMP_SERVICES_URL)

if(NOT TARGET OBS::file-updater)
  add_subdirectory("${CMAKE_SOURCE_DIR}/shared/file-updater" "${CMAKE_BINARY_DIR}/shared/file-updater")
endif()

find_package(jansson REQUIRED)

add_library(rtmp-services MODULE)
add_library(OBS::rtmp-services ALIAS rtmp-services)

target_sources(
  rtmp-services
  PRIVATE
    build/rtmp-services.core+.h.c
    rtmp-format-ver.h
    service-specific/amazon-ivs.h
    service-specific/dacast.h
    service-specific/nimotv.h
    service-specific/service-ingest.h
    service-specific/showroom.h
    service-specific/twitch.h
)

target_compile_definitions(
  rtmp-services
  PRIVATE SERVICES_URL="${RTMP_SERVICES_URL}" $<$<BOOL:${ENABLE_SERVICE_UPDATES}>:ENABLE_SERVICE_UPDATES>
)

target_link_libraries(rtmp-services PRIVATE OBS::libobs OBS::file-updater jansson::jansson)

# #2625: this target's conversions are fixed at the call sites, so the pins that
# hide them are not needed here. The global -Wno-conversion / -Wno-sign-conversion /
# -Wno-float-conversion only leave the flag lists once every target is ratcheted.
target_compile_options(
  rtmp-services
  PRIVATE "$<$<COMPILE_LANG_AND_ID:C,GNU,Clang,AppleClang>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)

# Remove once jansson has been fixed on obs-deps
target_link_options(rtmp-services PRIVATE $<$<PLATFORM_ID:Windows>:/IGNORE:4098>)

if(OS_WINDOWS)
  configure_file(cmake/windows/obs-module.rc.in rtmp-services.rc)
  target_sources(rtmp-services PRIVATE rtmp-services.rc)
endif()

set_target_properties_obs(rtmp-services PROPERTIES FOLDER plugins PREFIX "")
