cmake_minimum_required(VERSION 3.28...3.30)

find_package(X11 REQUIRED)

find_package(
  XCB
  REQUIRED XCB XFIXES RANDR SHM XINERAMA COMPOSITE
)

add_library(linux-capture MODULE)
add_library(OBS::capture ALIAS linux-capture)

target_sources(linux-capture PRIVATE build/linux-capture.nix+.h.c xcomposite-input.h xcursor-xcb.h xhelpers.h)

target_link_libraries(
  linux-capture
  PRIVATE OBS::libobs OBS::glad X11::X11 XCB::XCB XCB::XFIXES XCB::RANDR XCB::SHM XCB::XINERAMA XCB::COMPOSITE
)

# #2625: this target's conversions are fixed at the call sites, so the pins that
# hide them are not needed here. The global -Wno-conversion / -Wno-sign-conversion /
# -Wno-float-conversion only leave the flag lists once every target is ratcheted.
target_compile_options(
  linux-capture
  PRIVATE "$<$<COMPILE_LANG_AND_ID:C,GNU,Clang,AppleClang>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)

set_target_properties_obs(linux-capture PROPERTIES FOLDER plugins PREFIX "")
