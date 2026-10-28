target_link_libraries(
  libobs-objects
  PUBLIC
    "$<LINK_LIBRARY:FRAMEWORK,AppKit.framework>"
    "$<LINK_LIBRARY:FRAMEWORK,AudioToolbox.framework>"
    "$<LINK_LIBRARY:FRAMEWORK,AudioUnit.framework>"
    "$<LINK_LIBRARY:FRAMEWORK,Carbon.framework>"
    "$<LINK_LIBRARY:FRAMEWORK,Cocoa.framework>"
    "$<LINK_LIBRARY:FRAMEWORK,CoreAudio.framework>"
    "$<LINK_LIBRARY:FRAMEWORK,IOKit.framework>"
)

libobs_add_host_sources(MACOS)

foreach(target libobs-objects libobs)
  target_compile_options(${target} PUBLIC "$<$<NOT:$<COMPILE_LANGUAGE:Swift>>:-Wno-shorten-64-to-32>")
endforeach()

set_property(
  SOURCE build/libobs.macos-cocoa+.h.m build/libobs.ported-macos-cocoa+.h.m
  PROPERTY COMPILE_OPTIONS -fobjc-arc
)
set_property(TARGET ${LIBOBS_SHARED_TARGETS} PROPERTY FRAMEWORK TRUE)
set_property(TARGET libobs APPEND PROPERTY OBS_PUBLIC_HEADERS util/apple/cfstring-utils.h)
