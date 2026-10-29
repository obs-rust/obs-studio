cmake_minimum_required(VERSION 3.28...3.30)

add_library(happy-eyeballs OBJECT)
add_library(OBS::happy-eyeballs ALIAS happy-eyeballs)

target_sources(
  happy-eyeballs
  PRIVATE
    build/happy-eyeballs.core+.h.c
    $<$<PLATFORM_ID:Windows>:${CMAKE_CURRENT_SOURCE_DIR}/hosts/windows/happy-eyeballs-platform.h>
    $<$<NOT:$<PLATFORM_ID:Windows>>:${CMAKE_CURRENT_SOURCE_DIR}/hosts/posix/happy-eyeballs-platform.h>
  PUBLIC
    happy-eyeballs.h
    $<$<PLATFORM_ID:Windows>:${CMAKE_CURRENT_SOURCE_DIR}/hosts/windows/happy-eyeballs-socket.h>
    $<$<NOT:$<PLATFORM_ID:Windows>>:${CMAKE_CURRENT_SOURCE_DIR}/hosts/posix/happy-eyeballs-socket.h>
)

# The platform layer (Winsock or BSD sockets) lives in hosts/<os>/ so the
# shared files have no platform conditionals (#847).
target_include_directories(
  happy-eyeballs
  PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}" "${CMAKE_CURRENT_SOURCE_DIR}/hosts/$<IF:$<PLATFORM_ID:Windows>,windows,posix>"
)

target_link_libraries(happy-eyeballs PUBLIC OBS::libobs)

set_target_properties(happy-eyeballs PROPERTIES FOLDER deps POSITION_INDEPENDENT_CODE TRUE)

# #2625: this target's conversions are fixed at the call sites, so the pins that
# hide them are not needed here. The global -Wno-conversion / -Wno-sign-conversion /
# -Wno-float-conversion only leave the flag lists once every target is ratcheted.
target_compile_options(
  happy-eyeballs
  PRIVATE "$<$<COMPILE_LANG_AND_ID:C,AppleClang,Clang,GNU>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)
