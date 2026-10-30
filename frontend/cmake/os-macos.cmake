# Selected once by frontend/CMakeLists.txt. Macros retain caller scope and
# run at the original feature/source stages; including this file does no setup.

macro(obs_frontend_platform_qt)
  # Qt 6.11 Cocoa's default-icon focus callback needs a private adapter.
  if(Qt6_VERSION VERSION_GREATER_EQUAL 6.11 AND Qt6_VERSION VERSION_LESS 6.12)
    find_package(Qt6 REQUIRED COMPONENTS GuiPrivate)
    target_link_libraries(obs-studio PRIVATE Qt6::GuiPrivate)
  endif()

  # Qt's headers are third-party code, so the conversion ratchet (#2625) must not
  # report Qt's own inline code. The Ninja lanes get that for free: an imported
  # target's INTERFACE_INCLUDE_DIRECTORIES arrive as system includes. The Xcode
  # generator has no -isystem -- Modules/Platform/Darwin.cmake clears
  # CMAKE_INCLUDE_SYSTEM_FLAG_C and _CXX when XCODE is set -- so here the same
  # directories arrive as plain -I and -Wsign-conversion fires inside
  # <QtCore/qnamespace.h> (QKeyCombination) and <QtCore/qversiontagging.h>, which
  # stops every macOS and release lane (#2950).
  #
  # Pass those directories to clang by hand. A header is a system header when any
  # -isystem path finds it, even when -I lists the directory too, so CMake's own
  # -I is harmless (checked with clang on 2026-10-29).
  set(_obs_qt_system_includes "")
  foreach(
    _obs_qt_target
    IN
    ITEMS Qt6::Widgets Qt6::Svg Qt6::Xml Qt6::Network Qt6::Core Qt6::Gui Qt6::GuiPrivate
  )
    if(NOT TARGET ${_obs_qt_target})
      continue()
    endif()
    get_target_property(_obs_qt_dirs ${_obs_qt_target} INTERFACE_INCLUDE_DIRECTORIES)
    foreach(_obs_qt_dir IN LISTS _obs_qt_dirs)
      if(IS_DIRECTORY "${_obs_qt_dir}")
        list(APPEND _obs_qt_system_includes "${_obs_qt_dir}")
      endif()
    endforeach()
  endforeach()

  if(TARGET obs-studio AND _obs_qt_system_includes)
    list(REMOVE_DUPLICATES _obs_qt_system_includes)
    foreach(_obs_qt_dir IN LISTS _obs_qt_system_includes)
      target_compile_options(obs-studio PRIVATE "$<$<COMPILE_LANGUAGE:C,CXX,OBJC,OBJCXX>:-isystem${_obs_qt_dir}>")
    endforeach()
  endif()
endmacro()

macro(obs_frontend_platform_plugin_paths)
  # Use the shared absolute installation paths.
endmacro()

macro(obs_frontend_platform_plugin_manager)
  obs_frontend_add_sources(MACOS_PLUGIN_MANAGER)
endmacro()

macro(obs_frontend_platform_whatsnew)
  include(cmake/feature-macos-update.cmake)
  target_compile_definitions(obs-studio PRIVATE WHATSNEW_URL=MAC_WHATSNEW_URL)
endmacro()

macro(obs_frontend_platform_sources)
  obs_frontend_add_sources(POSIX_SIGNALS)
  include(cmake/feature-sparkle.cmake)

  obs_frontend_add_sources(MACOS)
  target_compile_options(obs-studio PRIVATE -Wno-quoted-include-in-framework-header -Wno-comma)

  list(APPEND _frontend_objcxx_compile_options -fobjc-arc -fmodules -fcxx-modules)

  set_property(
    SOURCE build/frontend.macos-arc-mm+.hpp.mm
    APPEND
    PROPERTY COMPILE_OPTIONS ${_frontend_objcxx_compile_options}
  )

  if(CMAKE_C_COMPILER_VERSION VERSION_GREATER_EQUAL 14.0.3)
    target_compile_options(obs-studio PRIVATE -Wno-error=unqualified-std-cast-call)
  endif()

  target_link_libraries(
    obs-studio
    PRIVATE
      "$<LINK_LIBRARY:FRAMEWORK,AppKit.framework>"
      "$<LINK_LIBRARY:FRAMEWORK,ApplicationServices.framework>"
      "$<LINK_LIBRARY:FRAMEWORK,AVFoundation.framework>"
  )
endmacro()
