# VST 2.x filter: closing the plug-in editor with its window's own close
# button leaves "Close Plug-in Interface" in the filter's properties
# (obsproject/obs-studio#13580). Builds the obs-vst module sources into a
# test copy of the module (CI configures with ENABLE_VST=OFF) and a small
# VST 2.x plug-in, then drives the editor on Qt's offscreen platform. Linux
# only (the plug-in is a plain shared object there); obs_startup needs a
# display, so it runs under xvfb-run and skips without one.
if(OS_LINUX AND (NOT DEFINED ENABLE_UI OR ENABLE_UI) AND (NOT DEFINED ENABLE_FRONTEND OR ENABLE_FRONTEND))
  find_package(Qt6 QUIET COMPONENTS Widgets)
  if(TARGET Qt6::Widgets)
    set(_vst_dir "${CMAKE_SOURCE_DIR}/plugins/obs-vst")

    add_library(test_vst_editor_close_plugin MODULE "${CMAKE_SOURCE_DIR}/test/cmocka/vst_editor_close_plugin.cpp")
    target_include_directories(test_vst_editor_close_plugin PRIVATE "${_vst_dir}/vst_header")
    set_target_properties(test_vst_editor_close_plugin PROPERTIES PREFIX "" CXX_VISIBILITY_PRESET hidden)

    add_library(
      test_vst_editor_close_module
      MODULE
      "${_vst_dir}/build/obs-vst.core+.hpp.cpp"
      "${_vst_dir}/build/obs-vst.linux+.hpp.cpp"
      "${_vst_dir}/headers/EditorWidget.h"
      "${_vst_dir}/headers/VSTPlugin.h"
    )
    target_include_directories(test_vst_editor_close_module PRIVATE "${_vst_dir}/vst_header" "${_vst_dir}/hosts/linux")
    target_link_libraries(test_vst_editor_close_module PRIVATE OBS::libobs Qt6::Widgets)
    set_target_properties(test_vst_editor_close_module PROPERTIES PREFIX "" AUTOMOC ON)

    # Executable and CTest metadata are in build/test-graph.json.
    unset(_vst_dir)
  endif()
endif()
