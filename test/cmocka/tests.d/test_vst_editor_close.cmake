# VST 2.x filter: closing the plug-in editor with its window's own close
# button leaves "Close Plug-in Interface" in the filter's properties
# (obsproject/obs-studio#13580). Builds the obs-vst module sources into a
# test copy of the module (CI configures with ENABLE_VST=OFF) and a small
# VST 2.x plug-in, then drives the editor on Qt's offscreen platform. Linux
# only (the plug-in is a plain shared object there); obs_startup needs a
# display, so it runs under xvfb-run and skips without one.
if(OS_LINUX)
  find_package(Qt6 QUIET COMPONENTS Widgets)
  if(TARGET Qt6::Widgets)
    set(_vst_dir "${CMAKE_SOURCE_DIR}/plugins/obs-vst")

    add_library(test_vst_editor_close_plugin MODULE vst_editor_close_plugin.cpp)
    target_include_directories(test_vst_editor_close_plugin PRIVATE "${_vst_dir}/vst_header")
    set_target_properties(test_vst_editor_close_plugin PROPERTIES PREFIX "" CXX_VISIBILITY_PRESET hidden)

    add_library(
      test_vst_editor_close_module
      MODULE
      "${_vst_dir}/EditorWidget.cpp"
      "${_vst_dir}/headers/EditorWidget.h"
      "${_vst_dir}/headers/VSTPlugin.h"
      "${_vst_dir}/linux/EditorWidget-linux.cpp"
      "${_vst_dir}/linux/VSTPlugin-linux.cpp"
      "${_vst_dir}/obs-vst.cpp"
      "${_vst_dir}/VSTPlugin.cpp"
    )
    target_include_directories(test_vst_editor_close_module PRIVATE "${_vst_dir}/vst_header")
    target_link_libraries(test_vst_editor_close_module PRIVATE OBS::libobs Qt6::Widgets)
    set_target_properties(test_vst_editor_close_module PROPERTIES PREFIX "" AUTOMOC ON)

    add_executable(test_vst_editor_close test_vst_editor_close.cpp)
    target_compile_definitions(
      test_vst_editor_close
      PRIVATE
        VST_MODULE_PATH="$<TARGET_FILE:test_vst_editor_close_module>"
        VST_DATA_PATH="${_vst_dir}/data"
        VST_TEST_PLUGIN_PATH="$<TARGET_FILE:test_vst_editor_close_plugin>"
    )
    target_include_directories(test_vst_editor_close PRIVATE ${CMOCKA_INCLUDE_DIR})
    target_link_libraries(test_vst_editor_close PRIVATE OBS::libobs Qt6::Widgets ${CMOCKA_LIBRARIES})
    add_dependencies(test_vst_editor_close test_vst_editor_close_module test_vst_editor_close_plugin)
    add_dependencies(cmocka-tests test_vst_editor_close)
    add_test(NAME test_vst_editor_close COMMAND test_vst_editor_close)
    set_tests_properties(test_vst_editor_close PROPERTIES TIMEOUT 60)
  endif()
endif()
