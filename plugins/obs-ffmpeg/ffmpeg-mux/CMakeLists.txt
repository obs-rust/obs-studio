cmake_minimum_required(VERSION 3.28...3.30)

option(ENABLE_FFMPEG_MUX_DEBUG "Enable FFmpeg-mux debugging" OFF)

find_package(FFmpeg 8.0 REQUIRED COMPONENTS avcodec avutil avformat)

add_executable(obs-ffmpeg-mux)
add_executable(OBS::ffmpeg-mux ALIAS obs-ffmpeg-mux)

target_sources(
  obs-ffmpeg-mux
  PRIVATE
    build/ffmpeg-mux.core+.h.c
    $<$<PLATFORM_ID:Windows>:build/ffmpeg-mux.windows+.h.c>
    $<$<NOT:$<PLATFORM_ID:Windows>>:build/ffmpeg-mux.posix+.h.c>
    ffmpeg-mux-host.h
    ffmpeg-mux.h
)

set_property(SOURCE build/ffmpeg-mux.core+.h.c APPEND PROPERTY COMPILE_OPTIONS "$<$<C_COMPILER_ID:MSVC>:/wd4996>")

target_link_libraries(
  obs-ffmpeg-mux
  PRIVATE OBS::libobs FFmpeg::avcodec FFmpeg::avutil FFmpeg::avformat $<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>
)

target_compile_definitions(obs-ffmpeg-mux PRIVATE $<$<BOOL:${ENABLE_FFMPEG_MUX_DEBUG}>:ENABLE_FFMPEG_MUX_DEBUG>)

if(OBS_FFMPEG_MUX_NAME)
  set_target_properties(obs-ffmpeg-mux PROPERTIES OUTPUT_NAME "${OBS_FFMPEG_MUX_NAME}")
endif()

set_target_properties_obs(obs-ffmpeg-mux PROPERTIES FOLDER plugins/obs-ffmpeg)

# #2625: this target's conversions are fixed at the call sites, so the pins that
# hide them are not needed here. The global -Wno-conversion / -Wno-sign-conversion /
# -Wno-float-conversion only leave the flag lists once every target is ratcheted.
target_compile_options(
  obs-ffmpeg-mux
  PRIVATE
    "$<$<COMPILE_LANG_AND_ID:C,AppleClang,Clang,GNU>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
    "$<$<COMPILE_LANG_AND_ID:CXX,AppleClang,Clang,GNU>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)
