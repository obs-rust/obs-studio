cmake_minimum_required(VERSION 3.28...3.30)

add_library(opts-parser OBJECT)
add_library(OBS::opts-parser ALIAS opts-parser)

target_sources(opts-parser PRIVATE build/opts-parser.core+.h.c PUBLIC opts-parser.h)

target_include_directories(opts-parser PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}")

target_link_libraries(opts-parser PUBLIC OBS::libobs)

set_target_properties(opts-parser PROPERTIES FOLDER deps)

# #2625: this target's conversions are fixed at the call sites, so the pins that
# hide them are not needed here. The global -Wno-conversion / -Wno-sign-conversion /
# -Wno-float-conversion only leave the flag lists once every target is ratcheted.
target_compile_options(
  opts-parser
  PRIVATE "$<$<COMPILE_LANG_AND_ID:C,AppleClang,Clang,GNU>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)
