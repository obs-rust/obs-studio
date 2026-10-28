add_library(libdshowcapture INTERFACE)
add_library(OBS::libdshowcapture ALIAS libdshowcapture)

target_sources(
  libdshowcapture
  INTERFACE src/dshowcapture.hpp
            src/source/capture-filter.cpp
            src/source/capture-filter.hpp
            src/source/device-vendor.cpp
            src/source/device.cpp
            src/source/device.hpp
            src/source/dshow-base.cpp
            src/source/dshow-base.hpp
            src/source/dshow-demux.cpp
            src/source/dshow-demux.hpp
            src/source/dshow-device-defs.hpp
            src/source/dshow-encoded-device.cpp
            src/source/dshow-enum.cpp
            src/source/dshow-enum.hpp
            src/source/dshow-formats.cpp
            src/source/dshow-formats.hpp
            src/source/dshow-media-type.cpp
            src/source/dshow-media-type.hpp
            src/source/dshowcapture.cpp
            src/source/dshowencode.cpp
            src/source/encoder.cpp
            src/source/encoder.hpp
            src/source/external/IVideoCaptureFilter.h
            src/source/log.cpp
            src/source/log.hpp
            src/source/output-filter.cpp
            src/source/output-filter.hpp
            src/external/capture-device-support/Library/EGAVResult.cpp
            src/external/capture-device-support/Library/ElgatoUVCDevice.cpp
            src/external/capture-device-support/Library/win/EGAVHIDImplementation.cpp
            src/external/capture-device-support/SampleCode/DriverInterface.cpp)

target_include_directories(
  libdshowcapture INTERFACE "${CMAKE_CURRENT_SOURCE_DIR}/src"
                            "${CMAKE_CURRENT_SOURCE_DIR}/src/external/capture-device-support/Library")

target_compile_definitions(libdshowcapture INTERFACE _UP_WINDOWS=1)

# /wd4005 and /wd4018 are the vendored-code suppressions this library has always
# carried. The MSVC /W4-only C4244/C4456/C4458 below belong to the same exempt set
# (#2626, #516): src/source/device.cpp:483 shadows its `defaultMT` local,
# src/source/encoder.cpp / output-filter.cpp shadow the class members `config`
# and `pin`, and external/capture-device-support/Library/ElgatoUVCDevice.cpp:218
# stores an int into a uint8_t. Fixing them would mean editing the vendored
# submodule, so the warnings are suppressed for the consumers that compile these
# sources (win-dshow, obs-virtualcam-module).
target_compile_options(
  libdshowcapture
  INTERFACE /wd4005 /wd4018 $<$<COMPILE_LANG_AND_ID:CXX,MSVC>:/wd4244;/wd4456;/wd4458>
)
