# Filters/Properties: changing two fields within 500 ms gave the first undo
# entry the second field's change too, so Undo, Undo, Redo restored the wrong
# settings (obsproject/obs-studio#10628). Builds the real properties view
# (shared/properties-view) and its Qt helpers without the frontend and runs it
# on Qt's offscreen platform, so it needs Qt Widgets and Qt Test but no display.
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
      test_properties_undo_coalesce
      test_properties_undo_coalesce.cpp
      "${_pv_dir}/double-slider.cpp"
      "${_pv_dir}/double-slider.hpp"
      "${_pv_dir}/properties-view.cpp"
      "${_pv_dir}/properties-view.hpp"
      "${_pv_dir}/properties-view.moc.hpp"
      "${_pv_dir}/spinbox-ignorewheel.cpp"
      "${_pv_dir}/spinbox-ignorewheel.hpp"
    )
    set_target_properties(test_properties_undo_coalesce PROPERTIES AUTOMOC ON)
    target_include_directories(
      test_properties_undo_coalesce
      PRIVATE ${CMOCKA_INCLUDE_DIR} "${_pv_dir}" "${CMAKE_SOURCE_DIR}/frontend/api"
    )
    target_link_libraries(
      test_properties_undo_coalesce
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
    add_dependencies(cmocka-tests test_properties_undo_coalesce)
    add_test(test_properties_undo_coalesce ${CMAKE_CURRENT_BINARY_DIR}/test_properties_undo_coalesce)
    unset(_pv_dir)
    unset(_qt_target)
  endif()
endif()
