cmake_minimum_required(VERSION 3.28...3.30)

option(ENABLE_V4L2 "Build OBS with v4l2 support" ON)
option(ENABLE_UDEV "Build linux-v4l2 with UDEV support" ON)

if(NOT ENABLE_V4L2)
  return()
endif()

find_package(Libv4l2 REQUIRED)
find_package(FFmpeg 8.0 REQUIRED COMPONENTS avcodec avutil avformat)

if(OS_FREEBSD OR OS_OPENBSD)
  set(CMAKE_REQUIRED_INCLUDES "/usr/local/include")
endif()
include(CheckCSourceCompiles)
set(V4L_TEST_SOURCE "#include<linux/videodev2.h>\nint main(){return 0;}")
check_c_source_compiles("${V4L_TEST_SOURCE}" HAVE_VIDEODEV2_HEADER)

if(NOT HAVE_VIDEODEV2_HEADER)
  message(FATAL_ERROR "Required system header <linux/videodev2.h> not found.")
endif()

add_library(linux-v4l2 MODULE)
add_library(OBS::v4l2 ALIAS linux-v4l2)

target_sources(linux-v4l2 PRIVATE build/linux-v4l2.nix+.h.c build/linux-v4l2.nix-output+.h.c)

target_link_libraries(
  linux-v4l2
  PRIVATE OBS::libobs Libv4l2::Libv4l2 FFmpeg::avcodec FFmpeg::avformat FFmpeg::avutil
)

if(ENABLE_UDEV)
  find_package(Libudev REQUIRED)
  target_sources(linux-v4l2 PRIVATE build/linux-v4l2.nix-udev+.h.c)

  target_link_libraries(linux-v4l2 PRIVATE Libudev::Libudev)
  target_compile_definitions(linux-v4l2 PRIVATE HAVE_UDEV)
endif()

# #2625: this target's conversions are fixed at the call sites, so the pins that
# hide them are not needed here. The global -Wno-conversion / -Wno-sign-conversion /
# -Wno-float-conversion only leave the flag lists once every target is ratcheted.
target_compile_options(
  linux-v4l2
  PRIVATE "$<$<COMPILE_LANG_AND_ID:C,GNU,Clang,AppleClang>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)

set_target_properties_obs(linux-v4l2 PROPERTIES FOLDER plugins PREFIX "")
