cmake_minimum_required(VERSION 3.28...3.30)

add_library(image-source MODULE)
add_library(OBS::image-source ALIAS image-source)

target_sources(
  image-source
  PRIVATE
    $<$<PLATFORM_ID:Darwin>:build/image-source.macos+.h.c>
    $<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:build/image-source.posix+.h.c>
    build/image-source.core+.h.c
    image-file-version.h
    image-source-host.h
    build/image-source.slideshow+.h.c
    slideshow-size.h
)

target_link_libraries(image-source PRIVATE OBS::libobs $<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>)

# #2625: this target's conversions are fixed at the call sites, so the pins that
# hide them are not needed here. The global -Wno-conversion / -Wno-sign-conversion /
# -Wno-float-conversion only leave the flag lists once every target is ratcheted.
target_compile_options(
  image-source
  PRIVATE "$<$<COMPILE_LANG_AND_ID:C,GNU,Clang,AppleClang>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)

if(OS_WINDOWS)
  configure_file(cmake/windows/obs-module.rc.in image-source.rc)
  target_sources(image-source PRIVATE build/image-source.windows+.h.c image-source.rc)
endif()

set_target_properties_obs(image-source PROPERTIES FOLDER plugins PREFIX "")
