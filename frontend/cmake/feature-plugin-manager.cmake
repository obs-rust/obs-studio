find_package(nlohmann_json 3.11 REQUIRED)

set(OBS_PLATFORM_INSTALL_PATH "${CMAKE_INSTALL_PREFIX}/")
set(OBS_PLATFORM_LIBRARY_PATH "${CMAKE_INSTALL_LIBDIR}/")
set(OBS_PLATFORM_DATA_PATH "${CMAKE_INSTALL_DATAROOTDIR}/")
set(OBS_PLATFORM_CORE_MODULE_PATH "${CMAKE_INSTALL_PREFIX}/${OBS_PLUGIN_DESTINATION}/core")
set(OBS_PLATFORM_CORE_DATA_PATH "${CMAKE_INSTALL_PREFIX}/${OBS_DATA_DESTINATION}/obs-modules/core")
set(OBS_PLATFORM_PLUGIN_MODULE_PATH "${CMAKE_INSTALL_PREFIX}/${OBS_PLUGIN_DESTINATION}/plugins")
set(OBS_PLATFORM_PLUGIN_DATA_PATH "${CMAKE_INSTALL_PREFIX}/${OBS_DATA_DESTINATION}/obs-modules/plugins")
# A relocatable build (the AppImage) runs from wherever it was unpacked, so the
# install prefix belongs to whatever else is installed there: a distribution's
# OBS keeps plugins for its own libobs in <prefix>/lib/obs-plugins. Its system
# plugin locations are its own tree, relative to the executable like its core
# modules (PluginModuleLoader_Linux.cpp.hpp resolves them).
if(ENABLE_RELOCATABLE AND OS_LINUX AND NOT OS_FLATPAK)
  set(OBS_PLATFORM_INSTALL_PATH "../")
  set(OBS_PLATFORM_PLUGIN_MODULE_PATH "../${OBS_PLUGIN_DESTINATION}/plugins")
  set(OBS_PLATFORM_PLUGIN_DATA_PATH "../${OBS_DATA_DESTINATION}/obs-modules/plugins")
endif()

target_sources(
  obs-studio
  PRIVATE
    build/frontend.plugin-manager-installedpluginrow+.hpp.cpp
    plugin-manager/InstalledPluginRow.hpp
    build/frontend.plugin-manager-pluginmanager+.hpp.cpp
    plugin-manager/PluginManager.hpp
    build/frontend.plugin-manager-pluginmanagerwindow+.hpp.cpp
    plugin-manager/PluginManagerWindow.hpp
    plugin-manager/PluginModuleLoader.hpp
)

if(OS_WINDOWS)
  target_sources(obs-studio PRIVATE build/frontend.plugin-manager-pluginmoduleloader_windows+.hpp.cpp)
elseif(OS_MACOS)
  target_sources(obs-studio PRIVATE build/frontend.plugin-manager-mm+.hpp.mm)
elseif(OS_FLATPAK)
  configure_file(plugin-manager/LoaderPaths_Flatpak.hpp.in LoaderPaths_Flatpak.hpp @ONLY)
  target_sources(obs-studio PRIVATE build/frontend.plugin-manager-flatpak+.hpp.cpp LoaderPaths_Flatpak.hpp)
elseif(OS_LINUX)
  configure_file(plugin-manager/LoaderPaths_Linux.hpp.in LoaderPaths_Linux.hpp @ONLY)
  target_sources(obs-studio PRIVATE build/frontend.plugin-manager-linux+.hpp.cpp LoaderPaths_Linux.hpp)
elseif(OS_FREEBSD OR OS_OPENBSD)
  configure_file(plugin-manager/LoaderPaths_BSD.hpp.in LoaderPaths_BSD.hpp @ONLY)
  target_sources(obs-studio PRIVATE build/frontend.plugin-manager-bsd+.hpp.cpp LoaderPaths_BSD.hpp)
endif()

target_link_libraries(obs-studio PRIVATE nlohmann_json::nlohmann_json)
