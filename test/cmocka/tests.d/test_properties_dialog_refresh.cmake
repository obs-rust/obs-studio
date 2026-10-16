# Properties: a refresh (obs_source_update_properties) while a field's file,
# font, color or list-entry dialog was open deleted the field's widgets under
# the dialog; picking a value then wrote to freed memory
# (obsproject/obs-studio#10423). Builds the real properties view
# (shared/properties-view) and its Qt helpers without the frontend and runs it
# on Qt's offscreen platform, so it needs Qt Widgets and Qt Test but no display.
# Built with ASan so a freed widget fails the test.
if(OS_LINUX)
  find_package(Qt6 QUIET COMPONENTS Widgets Test)
  if(TARGET Qt6::Widgets AND TARGET Qt6::Test)
    foreach(
      _qt_lib
      IN
      ITEMS wrappers plain-text-edit vertical-scroll-area slider-ignorewheel icon-label obs-spinbox
    )
      if(_qt_lib STREQUAL "wrappers")
        set(_qt_target OBS::qt-wrappers)
      else()
        set(_qt_target OBS::qt-${_qt_lib})
      endif()
      if(NOT TARGET ${_qt_target})
        add_subdirectory("${CMAKE_SOURCE_DIR}/shared/qt/${_qt_lib}" "${CMAKE_BINARY_DIR}/shared/qt/${_qt_lib}")
      endif()
    endforeach()
    set(_pv_dir "${CMAKE_SOURCE_DIR}/shared/properties-view")
    add_executable(
      test_properties_dialog_refresh
      test_properties_dialog_refresh.cpp
      "${_pv_dir}/double-slider.cpp"
      "${_pv_dir}/double-slider.hpp"
      "${_pv_dir}/properties-view.cpp"
      "${_pv_dir}/properties-view.hpp"
      "${_pv_dir}/properties-view.moc.hpp"
      "${_pv_dir}/spinbox-ignorewheel.cpp"
      "${_pv_dir}/spinbox-ignorewheel.hpp"
    )
    set_target_properties(test_properties_dialog_refresh PROPERTIES AUTOMOC ON)
    target_include_directories(
      test_properties_dialog_refresh
      PRIVATE ${CMOCKA_INCLUDE_DIR} "${_pv_dir}" "${CMAKE_SOURCE_DIR}/frontend/api"
    )
    target_link_libraries(
      test_properties_dialog_refresh
      PRIVATE
        OBS::libobs
        OBS::qt-wrappers
        OBS::qt-plain-text-edit
        OBS::qt-obs-spinbox
        OBS::qt-vertical-scroll-area
        OBS::qt-slider-ignorewheel
        OBS::qt-icon-label
        Qt6::Widgets
        Qt6::Test
        ${CMOCKA_LIBRARIES}
    )
    add_dependencies(cmocka-tests test_properties_dialog_refresh)
    obs_test_sanitize(test_properties_dialog_refresh address,undefined)
    add_test(test_properties_dialog_refresh ${CMAKE_CURRENT_BINARY_DIR}/test_properties_dialog_refresh)
    # Qt and fontconfig keep allocations until exit; only memory errors count.
    set_tests_properties(test_properties_dialog_refresh PROPERTIES ENVIRONMENT "ASAN_OPTIONS=detect_leaks=0")
    unset(_pv_dir)
    unset(_qt_target)
  endif()
endif()
