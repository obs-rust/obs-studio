# Plugin Manager: Safe Mode and --only-bundled-plugins showed the "plugins
# failed to load" dialog and listed every third-party plugin as Missing
# (obsproject/obs-studio#13948); removing a Missing entry made later rows act
# on the wrong plugin (obsproject/obs-studio#14005). Builds the real
# frontend/plugin-manager window, rows and PluginManager with Idian against
# stand-ins for the frontend app (plugin-manager-stubs/) and the libobs module
# calls, on Qt's offscreen platform: needs Qt Widgets and Svg, no display, no
# frontend build and no libobs core.
if(OS_LINUX)
  find_package(Qt6 QUIET COMPONENTS Widgets Svg)
  find_package(nlohmann_json 3.11 QUIET)
  if(TARGET Qt6::Widgets AND TARGET Qt6::Svg AND TARGET nlohmann_json::nlohmann_json)
    if(NOT TARGET OBS::idian)
      add_subdirectory("${CMAKE_SOURCE_DIR}/shared/qt/idian" "${CMAKE_BINARY_DIR}/shared/qt/idian")
    endif()
    set(_pm_dir "${CMAKE_SOURCE_DIR}/frontend/plugin-manager")
    # uic by hand: a second AUTOUIC target in this directory makes CMake give
    # both autogen targets test_output_timer's .ui header.
    qt_wrap_ui(_pm_ui "${CMAKE_SOURCE_DIR}/frontend/forms/PluginManagerWindow.ui")
    add_executable(
      test_plugin_manager
      test_plugin_manager.cpp
      "${CMAKE_SOURCE_DIR}/frontend/components/InfoChip.cpp"
      "${CMAKE_SOURCE_DIR}/frontend/components/InfoChip.hpp"
      "${_pm_dir}/InstalledPluginRow.cpp"
      "${_pm_dir}/InstalledPluginRow.hpp"
      "${_pm_dir}/PluginManager.cpp"
      "${_pm_dir}/PluginManager.hpp"
      "${_pm_dir}/PluginManagerWindow.cpp"
      "${_pm_dir}/PluginManagerWindow.hpp"
      ${_pm_ui}
    )
    set_target_properties(test_plugin_manager PROPERTIES AUTOMOC ON)
    # The stand-ins come first so <OBSApp.hpp>, <qt-wrappers.hpp> and
    # <widgets/OBSBasic.hpp> resolve to them.
    target_include_directories(
      test_plugin_manager
      BEFORE
      PRIVATE
        "${CMAKE_CURRENT_SOURCE_DIR}/plugin-manager-stubs"
        "${CMAKE_CURRENT_BINARY_DIR}"
        ${CMOCKA_INCLUDE_DIR}
        "${CMAKE_SOURCE_DIR}/frontend"
        "${CMAKE_SOURCE_DIR}/frontend/api"
        "$<TARGET_PROPERTY:OBS::libobs,INTERFACE_INCLUDE_DIRECTORIES>"
    )
    target_link_libraries(
      test_plugin_manager
      PRIVATE OBS::idian Qt6::Widgets nlohmann_json::nlohmann_json ${CMOCKA_LIBRARIES}
    )
    add_dependencies(cmocka-tests test_plugin_manager)
    add_test(test_plugin_manager ${CMAKE_CURRENT_BINARY_DIR}/test_plugin_manager)
    unset(_pm_dir)
    unset(_pm_ui)
  endif()
endif()
