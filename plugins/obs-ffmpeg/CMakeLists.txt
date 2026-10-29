cmake_minimum_required(VERSION 3.28...3.30)

option(ENABLE_FFMPEG_LOGGING "Enables obs-ffmpeg logging" OFF)
option(ENABLE_NEW_MPEGTS_OUTPUT "Use native SRT/RIST mpegts output" ON)

if(OS_LINUX OR OS_WINDOWS)
  option(ENABLE_FFMPEG_NVENC "Enable legacy FFmpeg NVENC encoder" OFF)
endif()

include(cmake/dependencies.cmake)

add_library(obs-ffmpeg MODULE)
add_library(OBS::ffmpeg ALIAS obs-ffmpeg)

add_subdirectory(ffmpeg-mux)

target_sources(
  obs-ffmpeg
  PRIVATE
    $<$<BOOL:${ENABLE_FFMPEG_LOGGING}>:build/obs-ffmpeg.logging+.h.c>
    $<$<BOOL:${ENABLE_FFMPEG_NVENC}>:build/obs-ffmpeg.nvenc+.h.c>
    $<$<BOOL:${ENABLE_NEW_MPEGTS_OUTPUT}>:build/obs-ffmpeg.mpegts+.h.c>
    $<$<BOOL:${ENABLE_NEW_MPEGTS_OUTPUT}>:obs-ffmpeg-rist.h>
    $<$<BOOL:${ENABLE_NEW_MPEGTS_OUTPUT}>:obs-ffmpeg-srt.h>
    $<$<BOOL:${ENABLE_NEW_MPEGTS_OUTPUT}>:obs-ffmpeg-url.h>
    $<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:build/obs-ffmpeg.vaapi+.h.c>
    $<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:build/obs-ffmpeg.vaapi-device-select+.h.c>
    $<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:vaapi-device-select.h>
    $<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:build/obs-ffmpeg.vaapi-probe-policy+.h.c>
    $<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:vaapi-probe-policy.h>
    $<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:vaapi-utils.h>
    build/obs-ffmpeg.core+.h.c
    build/obs-ffmpeg.mux+.h.c
    $<$<PLATFORM_ID:Darwin>:build/obs-ffmpeg.mux-host-macos+.h.c>
    $<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:build/obs-ffmpeg.mux-host-posix+.h.c>
    $<$<PLATFORM_ID:Windows>:build/obs-ffmpeg.mux-host-windows+.h.c>
    $<$<PLATFORM_ID:Windows>:build/obs-ffmpeg.host-windows+.h.c>
    $<$<PLATFORM_ID:Linux>:build/obs-ffmpeg.host-linux+.h.c>
    $<$<PLATFORM_ID:Darwin,FreeBSD,OpenBSD>:build/obs-ffmpeg.host-posix+.h.c>
    obs-ffmpeg-host.h
    obs-ffmpeg-mux-host.h
    build/obs-ffmpeg.output+.h.c
    build/obs-ffmpeg.source+.h.c
    build/obs-ffmpeg.video-encoders+.h.c
    obs-ffmpeg-compat.h
    obs-ffmpeg-formats.h
    obs-ffmpeg-mux.h
    obs-ffmpeg-output.h
)

# Each host's obs-ffmpeg-net-host.h, included by obs-ffmpeg-url.h (#853).
target_include_directories(
  obs-ffmpeg
  PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/hosts/$<IF:$<PLATFORM_ID:Windows>,windows,posix>"
)

target_compile_options(obs-ffmpeg PRIVATE $<$<COMPILE_LANG_AND_ID:C,AppleClang,Clang>:-Wno-shorten-64-to-32>)
target_compile_definitions(
  obs-ffmpeg
  PRIVATE
    $<$<BOOL:${ENABLE_FFMPEG_LOGGING}>:ENABLE_FFMPEG_LOGGING>
    $<$<BOOL:${ENABLE_FFMPEG_NVENC}>:ENABLE_FFMPEG_NVENC>
    $<$<BOOL:${ENABLE_NEW_MPEGTS_OUTPUT}>:NEW_MPEGTS_OUTPUT>
    $<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:LIBAVUTIL_VAAPI_AVAILABLE>
    $<$<BOOL:${OBS_FFMPEG_MUX_NAME}>:OBS_FFMPEG_MUX_NAME="${OBS_FFMPEG_MUX_NAME}">
)

target_link_libraries(
  obs-ffmpeg
  PRIVATE
    OBS::libobs
    OBS::media-playback
    OBS::opts-parser
    FFmpeg::avcodec
    FFmpeg::avfilter
    FFmpeg::avformat
    FFmpeg::avdevice
    FFmpeg::avutil
    FFmpeg::swscale
    FFmpeg::swresample
    $<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>
    $<$<PLATFORM_ID:Windows>:ws2_32>
    $<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:Libva::va>
    $<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:Libva::drm>
    $<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:Libpci::pci>
    $<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:Libdrm::Libdrm>
    $<$<BOOL:${ENABLE_NEW_MPEGTS_OUTPUT}>:Librist::Librist>
    $<$<BOOL:${ENABLE_NEW_MPEGTS_OUTPUT}>:Libsrt::Libsrt>
)

# #2625: this target's conversions are fixed at the call sites, so the pins that
# hide them are not needed here. The global -Wno-conversion / -Wno-sign-conversion /
# -Wno-float-conversion only leave the flag lists once every target is ratcheted.
target_compile_options(
  obs-ffmpeg
  PRIVATE "$<$<COMPILE_LANG_AND_ID:C,CXX,GNU,Clang,AppleClang>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)

if(OS_WINDOWS)
  if(CMAKE_VS_PLATFORM_NAME STREQUAL x64)
    target_sources(obs-ffmpeg PRIVATE build/obs-ffmpeg.amf-windows+.hpp.cpp texture-amf-opts.hpp)
    target_link_libraries(obs-ffmpeg PRIVATE AMF::AMF)
    target_compile_definitions(obs-ffmpeg PRIVATE OBS_FFMPEG_AMF)
  endif()

  configure_file(cmake/windows/obs-module.rc.in obs-ffmpeg.rc)
  target_sources(obs-ffmpeg PRIVATE obs-ffmpeg.rc)
endif()

set_target_properties_obs(obs-ffmpeg PROPERTIES FOLDER plugins/obs-ffmpeg PREFIX "")
